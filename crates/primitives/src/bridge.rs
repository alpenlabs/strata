//! Primitive data types related to the bridge.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
};

use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    secp256k1::{schnorr, PublicKey},
    Transaction, TxOut,
};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{errors::BridgeParseError, l1::SpendInfo};

/// The ID of an operator.
///
/// We define it as a type alias over [`u32`] instead of a newtype because we perform a bunch of
/// mathematical operations on it while managing the operator table.
pub type OperatorIdx = u32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublickeyTable(pub BTreeMap<OperatorIdx, PublicKey>);

impl TryFrom<BTreeMap<OperatorIdx, PublicKey>> for PublickeyTable {
    type Error = BridgeParseError;

    fn try_from(value: BTreeMap<OperatorIdx, PublicKey>) -> Result<Self, Self::Error> {
        for i in value.keys().skip(1) {
            // The table of `PublicKey`'s must be sorted by the `OperatorIdx` in order to generate a
            // deterministic aggregated pubkey in MuSig2. This is a sanity check since
            // we always expect `OperatorTable` to be sorted by `OperatorIdx`.
            if *i < (i - 1) {
                return Err(BridgeParseError::MalformedPublicKeyTable);
            }
        }

        Ok(Self(value))
    }
}

impl From<PublickeyTable> for Vec<PublicKey> {
    fn from(value: PublickeyTable) -> Self {
        value.0.values().copied().collect()
    }
}

impl From<PublickeyTable> for BTreeSet<PublicKey> {
    fn from(value: PublickeyTable) -> Self {
        value.0.values().fold(BTreeSet::new(), |mut set, pk| {
            set.insert(*pk);

            set
        })
    }
}

impl BorshSerialize for PublickeyTable {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Serialize the length of the BTreeMap
        BorshSerialize::serialize(&(self.0.len() as u32), writer)?;

        // Serialize each key-value pair
        for (operator_idx, public_key) in &self.0 {
            // Serialize the operator index
            BorshSerialize::serialize(operator_idx, writer)?;
            // Serialize the public key as a byte array (33 bytes for secp256k1 public keys)
            writer.write_all(&public_key.serialize())?;
        }
        Ok(())
    }
}

impl BorshDeserialize for PublickeyTable {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let len = u32::deserialize_reader(reader)? as usize;
        let mut map = BTreeMap::new();

        for _ in 0..len {
            // Deserialize the operator index
            let operator_idx = OperatorIdx::deserialize_reader(reader)?;
            // Deserialize the public key (read 33 bytes for secp256k1 compressed public key)
            let mut key_bytes = [0u8; 33];
            reader.read_exact(&mut key_bytes)?;
            // Convert the byte array back into a PublicKey
            let public_key = PublicKey::from_slice(&key_bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid public key")
            })?;
            // Insert into the BTreeMap
            map.insert(operator_idx, public_key);
        }

        Ok(PublickeyTable(map))
    }
}

impl<'a> Arbitrary<'a> for PublickeyTable {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Limit the number of entries in the BTreeMap to a practical size (e.g., 10)
        let num_entries = u.arbitrary_len::<OperatorIdx>()? % 10;

        // Create an empty BTreeMap
        let mut map = BTreeMap::new();

        // Populate the BTreeMap with random OperatorIdx and PublicKey pairs
        for _ in 0..num_entries {
            // Arbitrary OperatorIdx
            let operator_idx = OperatorIdx::arbitrary(u)?;

            // Generate a random 33-byte compressed public key
            let key_bytes = u.bytes(33)?;
            let public_key =
                PublicKey::from_slice(key_bytes).map_err(|_| arbitrary::Error::IncorrectFormat)?;

            // Insert into the BTreeMap
            map.insert(operator_idx, public_key);
        }

        // Return the PublickeyTable with the generated map
        Ok(PublickeyTable(map))
    }
}

/// Wrapper type to implement traits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchnorrSignature(schnorr::Signature);

impl From<SchnorrSignature> for schnorr::Signature {
    fn from(value: SchnorrSignature) -> Self {
        value.0
    }
}

impl From<schnorr::Signature> for SchnorrSignature {
    fn from(value: schnorr::Signature) -> Self {
        Self(value)
    }
}

impl<'a> Arbitrary<'a> for SchnorrSignature {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let data: [u8; 64] = u.arbitrary()?; // Generate a 64-byte array
        let signature =
            schnorr::Signature::from_slice(&data).map_err(|_| arbitrary::Error::IncorrectFormat)?; // Handle potential invalid signatures
        Ok(SchnorrSignature(signature))
    }
}

impl BorshSerialize for SchnorrSignature {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.0.as_ref())?; // Serialize the inner schnorr::Signature
        Ok(())
    }
}

impl BorshDeserialize for SchnorrSignature {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut data = [0u8; 64];
        reader.read_exact(&mut data)?;
        schnorr::Signature::from_slice(&data)
            .map(SchnorrSignature)
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid schnorr signature")
            })
    }
}

/// The table of signatures collected so far for each input in a bridge transaction from each
/// required operator.
pub type CollectedSigs = Vec<BTreeMap<OperatorIdx, SchnorrSignature>>;

/// A container that encapsulates all the information necessary to produce a
/// valid signature for a transaction in the bridge.
#[derive(Debug, Clone)]
pub struct TxSigningData {
    /// The unsigned transaction (with the `script_sig` and `witness` fields not set).
    pub unsigned_tx: Transaction,

    /// The list of witness elements required to spend each input in the unsigned transaction
    /// respectively.
    pub spend_infos: Vec<SpendInfo>,

    /// The list of prevouts for each input in the unsigned transaction respectively.
    pub prevouts: Vec<TxOut>,
}

/// Information regarding the signature which includes the schnorr signature itself as well as the
/// pubkey of the signer so that the signature can be verified at the callsite (given a particular
/// message that was signed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// The schnorr signature for a given message.
    signature: SchnorrSignature,

    /// The index of the operator that can be used to query the corresponding pubkey.
    signer_index: OperatorIdx,
}

impl SignatureInfo {
    /// Create a new [`SignatureInfo`].
    pub fn new(signature: SchnorrSignature, signer_index: OperatorIdx) -> Self {
        Self {
            signature,
            signer_index,
        }
    }

    /// Get the schnorr signature.
    pub fn signature(&self) -> &SchnorrSignature {
        &self.signature
    }

    /// Get the index of the signer (operator).
    pub fn signer_index(&self) -> &OperatorIdx {
        &self.signer_index
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use borsh::{BorshDeserialize, BorshSerialize};

    use super::PublickeyTable;

    #[test]
    fn test_publickeytable_serialize_deserialize() {
        // Create a sample PublickeyTable
        let mut map = BTreeMap::new();
        map.insert(1, generate_public_key());
        map.insert(2, generate_public_key());
        let table = PublickeyTable(map);

        // Serialize the table
        let mut serialized = vec![];
        let result = table.serialize(&mut serialized);
        assert!(
            result.is_ok(),
            "borsh serialization should work but got error: {}",
            result.err().unwrap()
        );

        // Deserialize the table
        let deserialized: PublickeyTable =
            PublickeyTable::try_from_slice(&serialized).expect("Deserialization of PublickeyTable");

        // Ensure the deserialized table matches the original
        assert_eq!(table, deserialized);
    }

    #[test]
    fn test_empty_publickeytable_serialize_deserialize() {
        // Test with an empty PublickeyTable
        let table = PublickeyTable(BTreeMap::new());

        // Serialize the table
        let mut serialized = vec![];
        let result = table.serialize(&mut serialized);
        assert!(
            result.is_ok(),
            "serialization of empty publickeytable should work but got: {}",
            result.err().unwrap()
        );

        // Deserialize the table
        let deserialized: PublickeyTable = PublickeyTable::try_from_slice(&serialized)
            .expect("Deserialization of empty PublickeyTable");

        // Ensure the deserialized table matches the original (which is empty)
        assert_eq!(table, deserialized);
    }

    #[test]
    fn test_publickeytable_invalid_data() {
        // Create some invalid serialized data (wrong size for the public key)
        let invalid_data: Vec<u8> = vec![0, 0, 0, 1, 1, 2, 3]; // Not valid serialized format

        // Try deserializing the invalid data
        let result = PublickeyTable::try_from_slice(&invalid_data);

        // Ensure deserialization fails
        assert!(result.is_err());
    }

    // Helper function to create a random secp256k1 PublicKey
    fn generate_public_key() -> PublicKey {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x01; 32]).expect("32 bytes, within curve order");
        PublicKey::from_secret_key(&secp, &secret_key)
    }
}
