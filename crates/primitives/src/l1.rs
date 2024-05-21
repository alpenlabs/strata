use crate::buf::Buf32;

/// Reference to a transaction in a block.  This is the block index and the
/// position of the transaction in the block.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct L1TxRef(u32, u32);

/// Merkle proof for a TXID within a block.
// TODO rework this, make it possible to generate proofs, etc.
#[derive(Clone, Debug)]
pub struct L1TxProof {
    position: u32,
    cohashes: Vec<Buf32>,
}

/// Tx body with a proof.
#[derive(Clone, Debug)]
pub struct L1Tx {
    proof: L1TxProof,
    tx: Vec<u8>,
}
