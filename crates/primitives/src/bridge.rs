//! Primitive data types related to the bridge.

use std::{
    collections::BTreeMap,
    io::{Read, Write},
};

use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    key::rand,
    secp256k1::{schnorr, PublicKey, SecretKey},
    Transaction, TxOut,
};
use borsh::{BorshDeserialize, BorshSerialize};
use musig2::{BinaryEncoding, NonceSeed, PartialSignature, PubNonce, SecNonce};
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
//
// NOTE: this type is no longer used in the codebase but keeping it around just in case.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Musig2PartialSig(PartialSignature);

impl From<PartialSignature> for Musig2PartialSig {
    fn from(value: PartialSignature) -> Self {
        Self(value)
    }
}

impl Musig2PartialSig {
    pub fn inner(&self) -> &PartialSignature {
        &self.0
    }
}

impl BorshSerialize for Musig2PartialSig {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let serialized = self.0.serialize();

        writer.write_all(&serialized)
    }
}

impl BorshDeserialize for Musig2PartialSig {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        // Buffer size for 32-byte PartialSignature
        let mut partial_sig_bytes = [0u8; 32];
        reader.read_exact(&mut partial_sig_bytes)?; // Read exactly 32 bytes

        // Create PartialSignature from bytes
        let partial_sig = PartialSignature::from_slice(&partial_sig_bytes[..]).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid PartialSignature")
        })?;

        Ok(Self(partial_sig))
    }
}

impl<'a> Arbitrary<'a> for Musig2PartialSig {
    fn arbitrary(_u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let secret_key = SecretKey::new(&mut rand::thread_rng());

        // Create a PartialSignature from the secret key bytes
        let partial_sig = PartialSignature::from_slice(secret_key.as_ref())
            .map_err(|_| arbitrary::Error::IncorrectFormat)?;

        Ok(Self(partial_sig))
    }
}

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
#[derive(Debug, Clone, Copy, Arbitrary, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// The schnorr signature for a given message.
    partial_sig: Musig2PartialSig,

    /// The index of the operator that can be used to query the corresponding pubkey.
    signer_index: OperatorIdx,
}

impl SignatureInfo {
    /// Create a new [`SignatureInfo`].
    pub fn new(partial_sig: Musig2PartialSig, signer_index: OperatorIdx) -> Self {
        Self {
            partial_sig,
            signer_index,
        }
    }

    /// Get the schnorr signature.
    pub fn signature(&self) -> &Musig2PartialSig {
        &self.partial_sig
    }

    /// Get the index of the signer (operator).
    pub fn signer_index(&self) -> &OperatorIdx {
        &self.signer_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Musig2PubNonce(PubNonce);

impl Musig2PubNonce {
    pub fn inner(&self) -> &PubNonce {
        &self.0
    }
}

impl From<PubNonce> for Musig2PubNonce {
    fn from(value: PubNonce) -> Self {
        Self(value)
    }
}

impl BorshSerialize for Musig2PubNonce {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.0.serialize())?;

        Ok(())
    }
}

impl BorshDeserialize for Musig2PubNonce {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // Manually deserialize PubNonce (assume PubNonce has a `from_bytes` method)
        let mut nonce_bytes = [0u8; 66]; // same as uncompressed `secp256k1::PublicKey`
        reader.read_exact(&mut nonce_bytes)?;
        let nonce = PubNonce::from_bytes(&nonce_bytes).map_err(|_e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid pubnonce")
        })?;

        Ok(Self(nonce))
    }
}

impl<'a> Arbitrary<'a> for Musig2PubNonce {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut nonce_seed_bytes = [0u8; 32];
        u.fill_buffer(&mut nonce_seed_bytes)?;
        let nonce_seed = NonceSeed::from(nonce_seed_bytes);

        let sec_nonce = SecNonce::build(nonce_seed).build();
        let pub_nonce = sec_nonce.public_nonce();

        Ok(Self(pub_nonce))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Musig2SecNonce(SecNonce);

impl Musig2SecNonce {
    pub fn inner(&self) -> &SecNonce {
        &self.0
    }
}

impl From<SecNonce> for Musig2SecNonce {
    fn from(value: SecNonce) -> Self {
        Self(value)
    }
}

impl BorshSerialize for Musig2SecNonce {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.0.to_bytes())
    }
}

impl BorshDeserialize for Musig2SecNonce {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut sec_nonce_bytes = [0u8; 64];
        reader.read_exact(&mut sec_nonce_bytes)?;

        // Create SecNonce from bytes
        let sec_nonce = SecNonce::from_bytes(&sec_nonce_bytes).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SecNonce")
        })?;

        Ok(Musig2SecNonce(sec_nonce))
    }
}

impl<'a> Arbitrary<'a> for Musig2SecNonce {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate a random nonce seed (32 bytes)
        let mut nonce_seed_bytes = [0u8; 32];
        u.fill_buffer(&mut nonce_seed_bytes)?;
        let nonce_seed = NonceSeed::from(nonce_seed_bytes);

        let sec_nonce = SecNonce::build(nonce_seed).build();

        Ok(Musig2SecNonce(sec_nonce))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use arbitrary::{Arbitrary, Unstructured};
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use borsh::{BorshDeserialize, BorshSerialize};

    use super::{Musig2PubNonce, PublickeyTable};
    use crate::bridge::{Musig2PartialSig, Musig2SecNonce};

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

    #[test]
    fn test_borsh_serialization_musig2_partial_sig() {
        let raw_bytes = vec![0u8; 1024];
        let mut u = Unstructured::new(&raw_bytes);

        // Generate a random Musig2PartialSig using Arbitrary
        let musig2_partial_sig = Musig2PartialSig::arbitrary(&mut u);
        assert!(
            musig2_partial_sig.is_ok(),
            "should be able to generate musig2 partial sig but got: {}",
            musig2_partial_sig.err().unwrap()
        );

        let musig2_partial_sig = musig2_partial_sig.unwrap();

        // Serialize Musig2PartialSig using Borsh
        let mut serialized_sig = vec![];
        let result = musig2_partial_sig.serialize(&mut serialized_sig);

        assert!(
            result.is_ok(),
            "serialization of partial sig should work but got error: {}",
            result.err().unwrap()
        );

        // Ensure we wrote exactly 32 bytes
        assert_eq!(
            serialized_sig.len(),
            32,
            "serialized PartialSignature should be 32 bytes but got {} bytes",
            serialized_sig.len()
        );

        // Deserialize Musig2PartialSig using Borsh
        let deserialized_sig: Musig2PartialSig =
            Musig2PartialSig::deserialize(&mut &serialized_sig[..])
                .expect("deserialization should work");

        // Ensure the original and deserialized signatures are the same
        assert_eq!(
            deserialized_sig.0, musig2_partial_sig.0,
            "deserialized and original MuSig2 partial sigs should be the same"
        );
    }

    #[test]
    fn test_borsh_serialization_of_pub_nonce() {
        // Create a buffer of random bytes for generating a random NonceTable
        let raw_bytes = vec![0u8; 1024];
        let mut u = Unstructured::new(&raw_bytes);

        // Generate a random NonceTable using the Arbitrary implementation
        let orig_nonce =
            Musig2PubNonce::arbitrary(&mut u).expect("Failed to generate arbitrary NonceTable");

        // Serialize the PubNonce
        let mut serialized_nonce = vec![];
        let result = orig_nonce.serialize(&mut serialized_nonce);

        assert!(
            result.is_ok(),
            "serialization should work but got: {}",
            result.err().unwrap()
        );

        // Deserialize the PubNonce
        let deserialized_nonce: Musig2PubNonce =
            Musig2PubNonce::deserialize(&mut &serialized_nonce[..])
                .expect("deserialization of PubNonce should work");

        // Assert that the serialized and deserialized PubNonce values match
        assert_eq!(
            deserialized_nonce, orig_nonce,
            "Deserialized PubNonce does not match the original",
        );
    }

    #[test]
    fn test_borsh_serialization_sec_nonce() {
        // Create a buffer of random bytes for generating a random Musig2SecNonce
        let raw_bytes = vec![0u8; 1024];
        let mut u = Unstructured::new(&raw_bytes);

        // Generate a random Musig2SecNonce using Arbitrary
        let musig2_sec_nonce = Musig2SecNonce::arbitrary(&mut u);
        assert!(
            musig2_sec_nonce.is_ok(),
            "should be able to generate musig2 sec nonce but got: {}",
            musig2_sec_nonce.err().unwrap()
        );

        let musig2_sec_nonce = musig2_sec_nonce.unwrap();

        // Serialize Musig2SecNonce using Borsh
        let mut serialized_secnonce = vec![];
        let result = musig2_sec_nonce.serialize(&mut serialized_secnonce);

        assert!(
            result.is_ok(),
            "serialization of secnonce should work but got error: {}",
            result.err().unwrap()
        );

        assert_eq!(
            serialized_secnonce.len(),
            64,
            "secnonce should have a length of 66"
        );
        // Deserialize Musig2SecNonce using Borsh
        let deserialized_secnonce: Musig2SecNonce =
            Musig2SecNonce::deserialize(&mut &serialized_secnonce[..])
                .expect("deserialization should work");

        // Assert that the serialized and deserialized SecNonce values match
        assert_eq!(
            deserialized_secnonce, musig2_sec_nonce,
            "Deserialized SecNonce does not match the original"
        );
    }

    // Helper function to create a random secp256k1 PublicKey
    fn generate_public_key() -> PublicKey {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x01; 32]).expect("32 bytes, within curve order");
        PublicKey::from_secret_key(&secp, &secret_key)
    }
}
