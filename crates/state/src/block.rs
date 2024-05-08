use alpen_vertex_primitives::prelude::*;

use crate::l1::L1HeaderPayload;

/// ID of an L2 block, usually the hash of its root header.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L2BlockId(Buf32);

/// Block header that forms the chain we use to reach consensus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockHeader {
    /// Block index, obviously.
    block_idx: u64,

    /// Hash of the previous block, to form the blockchain.
    prev_block: L2BlockId,

    /// Hash of the L1 payload header.
    l1_payload_header_hash: Buf32,

    /// Hash of the exec payload header.
    exec_payload_header_hash: Buf32,

    /// State root that commits to the overall state of the rollup, commits to
    /// both the CL state and EL state.
    state_root: Buf32,

    /// Signature from this block's proposer.
    signature: Buf64,
}

/// Container for additional messages that we've observed from the L1, if there
/// are any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct L1PayloadHeader {
    /// New headers that we've seen from L1 that we didn't see in the previous
    /// L2 block.
    new_l1_headers: Vec<L1HeaderPayload>,
    // TODO forced inclusion messages?
    // TODO withdrawal messages?
}

/// Information relating to the EL payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecPayloadHeader {
    /// Commitment to the payload, maybe this should have a different structure.
    payload_commitment: Buf32,

    /// State commitment of the EL state.
    el_state_root: Buf32,
}
