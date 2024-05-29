use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::buf::Buf32;

/// Reference to a transaction in a block.  This is the block index and the
/// position of the transaction in the block.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct L1TxRef(u64, u32);

/// Merkle proof for a TXID within a block.
// TODO rework this, make it possible to generate proofs, etc.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1TxProof {
    position: u32,
    cohashes: Vec<Buf32>,
}

impl L1TxProof {
    pub fn new(position: u32, cohashes: Vec<Buf32>) -> Self {
        Self { position, cohashes }
    }
}

/// Tx body with a proof.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1Tx {
    proof: L1TxProof,
    tx: Vec<u8>,
}

impl L1Tx {
    pub fn new(proof: L1TxProof, tx: Vec<u8>) -> Self {
        Self { proof, tx }
    }
}

impl Into<(u64, u32)> for L1TxRef {
    fn into(self) -> (u64, u32) {
        (self.0, self.1)
    }
}

impl From<(u64, u32)> for L1TxRef {
    fn from(value: (u64, u32)) -> Self {
        Self(value.0, value.1)
    }
}
