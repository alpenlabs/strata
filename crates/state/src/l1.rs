use alpen_vertex_primitives::prelude::*;

/// ID of an L1 block, usually the hash of its header.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1BlockId(Buf32);

/// Represents a serialized L1 header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct L1HeaderPayload {
    /// Index in the L1 chain.  This helps us in case there's reorgs that the L2
    /// chain observes.
    idx: u64,

    /// Serialized header.  For Bitcoin this is always 80 bytes.
    buf: Vec<u8>,
}

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
