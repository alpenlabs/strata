//! Handle to inspect the current CSM state and wait for updates when there are
//! any.

use alpen_vertex_state::{client_state::ClientState, id::L2BlockId};

#[derive(Clone, Debug, Default)]
pub struct CsmStatus {
    /// Index of the last sync event.
    pub last_sync_ev_idx: u64,

    /// Chain tip's block ID.
    pub chain_tip_blkid: Option<L2BlockId>,

    /// Finalized block ID.
    pub finalized_blkid: Option<L2BlockId>,
}

impl CsmStatus {
    pub fn set_last_sync_ev_idx(&mut self, idx: u64) {
        self.last_sync_ev_idx = idx;
    }

    pub fn update_from_client_state(&mut self, state: &ClientState) {
        if let Some(ss) = state.sync() {
            self.chain_tip_blkid = Some(*ss.chain_tip_blkid());
            self.finalized_blkid = Some(*ss.finalized_blkid());
        } else {
            self.chain_tip_blkid = None;
            self.finalized_blkid = None;
        }
    }
}
