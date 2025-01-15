//! Handle to inspect the current CSM state and wait for updates when there are
//! any.

use crate::{client_state::ClientState, epoch::EpochCommitment, id::L2BlockId};

#[derive(Clone, Debug, Default)]
pub struct CsmStatus {
    /// Index of the last sync event.
    pub last_sync_ev_idx: u64,

    /// Finalized block ID.
    pub finalized_epoch: Option<EpochCommitment>,
}

impl CsmStatus {
    pub fn set_last_sync_ev_idx(&mut self, idx: u64) {
        self.last_sync_ev_idx = idx;
    }

    pub fn update_from_client_state(&mut self, state: &ClientState) {
        if let Some(ss) = state.sync() {
            self.finalized_epoch = Some(ss.get_epoch_commitment());
        } else {
            self.finalized_epoch = None;
        }
    }
}
