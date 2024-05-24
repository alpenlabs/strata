use alpen_vertex_primitives::prelude::*;

/// Succinct commitment to relevant EL block data.
// This ended up being the same as the EL payload types in the state crate,
// should we consolidate?
#[derive(Clone, Debug)]
pub struct ExecPayloadData {
    /// Payload commitment, probably a hash of the EL block header.
    payload_commitment: Buf32,

    /// Payload state root that we can make commitments to and whatnot.
    state_root: Buf32,

    /// Withdrawals initiated from within EL to L1.  This might be generalized
    /// to permit more types of EL->L1 operations.
    new_el_withdrawals: Vec<WithdrawData>,
}

/// L1 withdrawal data.
#[derive(Clone, Debug)]
pub struct WithdrawData {
    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Schnorr pubkey for the taproot output we're going to generate.
    dest_addr: Buf64,
}

/// Environment state from the CL that we pass into the EL for the payload we're
/// producing.  Maybe this should also have L1 headers or something?
#[derive(Clone, Debug)]
pub struct PayloadEnv {
    /// Timestamp we're attesting this block was created on.
    timestamp: u64,

    /// State root of the previous CL block.
    prev_state_root: Buf32,

    /// Safe L1 block we're exposing into the EL that's not likely to reorg.
    safe_l1_block: Buf32,

    /// Operations we're pushing into the EL for processing.
    el_ops: Vec<Op>,
}

/// Operation the CL pushes into the EL to perform as part of the block it's
/// producing.
#[derive(Clone, Debug)]
pub enum Op {
    /// Deposit some amount.
    Deposit(ELDepositData),
}

#[derive(Clone, Debug)]
pub struct ELDepositData {
    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Dest addr encoded in a portable format, assumed to be valid but must be
    /// checked by EL before committing to building block.
    dest_addr: Vec<u8>,
}
