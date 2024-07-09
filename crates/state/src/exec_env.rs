//! Types relating to the state of an execution environment in the CL state.

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;

use crate::{bridge_ops, da_blob, exec_update, forced_inclusion};

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct ExecEnvState {
    /// The last processed exec update, which we've checked to be valid.  We may
    /// not have seen its DA blobs on the L1 yet.
    last_update: exec_update::ExecUpdate,

    /// DA blobs that have been indicated by a exec update, but haven't been
    /// seen on the corresponding DA layer yet.
    ///
    /// This must always be sorted.
    waiting_da_blobs: Vec<da_blob::BlobSpec>,

    /// Deposits that have been queued by something but haven't been accepted in
    /// an update yet.  The sequencer should be processing these as soon as
    /// possible.
    pending_deposits: Vec<bridge_ops::Deposit>,

    /// Forced inclusions that have been accepted by the CL but not processed by
    /// a CL payload yet.
    // TODO This is a stub, we don't support these yet and should assert it to
    // be empty.
    pending_forced_incls: Vec<forced_inclusion::ForcedInclusion>,
}

impl ExecEnvState {
    pub fn update_idx(&self) -> u64 {
        self.last_update.input().update_idx()
    }

    pub fn cur_state_root(&self) -> &Buf32 {
        self.last_update.output().new_state()
    }
}
