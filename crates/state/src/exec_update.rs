//! Chain data types relating to the CL's updating view of an execution
//! environment's state.  For now the EVM EL is the only execution environment.

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;

use crate::{bridge_ops, da_blob};

/// Full update payload containing inputs and outputs to an EE state update.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct ExecUpdate {
    /// Inputs used to construct the call to the EL executor, or provided to the
    /// proof.
    input: UpdateInput,

    /// Conceptual "outputs" from the state transition that we verify either in
    /// the proof or by asking the EL.
    output: UpdateOutput,
}

impl ExecUpdate {
    pub fn input(&self) -> &UpdateInput {
        &self.input
    }

    pub fn output(&self) -> &UpdateOutput {
        &self.output
    }
}

/// Contains the explicit inputs to the STF.  Implicit inputs are determined
/// from the CL's exec env state.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct UpdateInput {
    /// Update index.  This is incremented exactly 1.  This is to handle the
    /// future possible cases where we skip CL blocks and provide a monotonic
    /// ordering of EL states.
    update_idx: u64,

    /// Merkle tree root of the contents of the EL payload, in the order it was
    /// expressed in the block.
    entries_root: Buf32,

    /// Buffer of any other payload data.  This is used with the other fields
    /// here to construct the full EVM header payload.
    extra_payload: Vec<u8>,
}

impl UpdateInput {
    pub fn update_idx(&self) -> u64 {
        self.update_idx
    }
}

/// Conceptual "outputs" from the state transition.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct UpdateOutput {
    /// New state root for the update.  This is not just the inner EL payload,
    /// but also any extra bookkeeping we need across multiple.
    new_state: Buf32,

    /// Bridge withdrawal intents.
    withdrawals: Vec<bridge_ops::WithdrawalIntent>,

    /// DA blobs that we expect to see on L1.  This may be empty, probably is
    /// only set near the end of the range of blocks in a batch since we only
    /// assert these in a per-batch frequency.
    da_blobs: Vec<da_blob::BlobSpec>,
}

impl UpdateOutput {
    pub fn new_state(&self) -> &Buf32 {
        &self.new_state
    }

    pub fn withdrawals(&self) -> &[bridge_ops::WithdrawalIntent] {
        &self.withdrawals
    }

    pub fn da_blobs(&self) -> &[da_blob::BlobSpec] {
        &self.da_blobs
    }
}
