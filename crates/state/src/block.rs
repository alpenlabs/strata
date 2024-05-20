use alpen_vertex_primitives::l1::L1Tx;
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
    // TODO figure out the structure of this
    state_root: Buf32,

    /// Signature from this block's proposer.
    signature: Buf64,
}

/// Container for additional messages that we've observed from the L1, if there
/// are any.
#[derive(Clone, Debug)]
pub struct L1PayloadHeader {
    /// New headers that we've seen from L1 that we didn't see in the previous
    /// L2 block.
    new_l1_headers: Vec<L1HeaderPayload>,

    /// Deposit initiation transactions.
    deposits: Vec<L1Tx>,
}

/// Information relating to the EL payloads.
#[derive(Clone, Debug)]
pub struct ExecPayloadHeader {
    /// Commitment to the payload.  This might be the EVM EL block header or
    /// maybe it's the full block.
    payload_commitment: Buf32,

    /// State commitment of the EL state.
    el_state_root: Buf32,

    /// Withdrawals that were initiated from the EL payload.
    new_el_withdraws: Vec<WithdrawData>,
}

/// Data emitted by EL exec for a withdraw request.
#[derive(Clone, Debug)]
pub struct WithdrawData {
    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Schnorr pubkey for the taproot output we're going to generate.
    dest_addr: Buf64,
}
