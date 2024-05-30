use arbitrary::Arbitrary;
use bitcoin::{consensus::serialize, Block, MerkleBlock, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::buf::Buf32;

/// Reference to a transaction in a block.  This is the block index and the
/// position of the transaction in the block.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct L1TxRef(u64, u32);

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

    pub fn position(&self) -> u32 {
        self.position
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

    pub fn proof(&self) -> &L1TxProof {
        &self.proof
    }

    pub fn tx_data(&self) -> &[u8] {
        &self.tx
    }
}

impl From<((u32, &Transaction), &Block)> for L1Tx {
    fn from(((idx, tx), block): ((u32, &Transaction), &Block)) -> Self {
        // TODO: construct cohashes properly
        let _merkleblock =
            MerkleBlock::from_block_with_predicate(block, |&x| x == tx.compute_txid());
        // TODO: continue
        let proof = L1TxProof {
            position: idx,
            cohashes: vec![],
        };
        let tx = serialize(tx);
        Self { proof, tx }
    }
}
