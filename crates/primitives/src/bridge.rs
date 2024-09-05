//! Primitive data types related to the bridge.

use std::collections::{BTreeMap, BTreeSet};

use arbitrary::{Arbitrary, Unstructured};
use bitcoin::secp256k1::{schnorr, PublicKey};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// The ID of an operator.
///
/// We define it as a type alias over [`u32`] instead of a newtype because we perform a bunch of
/// mathematical operations on it while managing the operator table.
pub type OperatorIdx = u32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublickeyTable(pub BTreeMap<OperatorIdx, PublicKey>);

impl From<Vec<PublicKey>> for PublickeyTable {
    fn from(value: Vec<PublicKey>) -> Self {
        let mut table: BTreeMap<OperatorIdx, PublicKey> = BTreeMap::new();

        for (i, pk) in value.iter().enumerate() {
            table.insert(i as u32, *pk);
        }

        Self(table)
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

/// Wrapper type to implement traits on [`schnorr::Signature`].
#[derive(Debug, Clone, PartialEq, Eq)]
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

// Implement Arbitrary for SchnorrSignature using the arbitrary crate
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
