//! Container for chain status.

use std::sync::Arc;

use strata_primitives::{epoch::EpochCommitment, l2::L2BlockCommitment};
use strata_state::chain_state::Chainstate;

/// Describes FCM state.
#[derive(Copy, Clone, Debug)]
pub struct ChainSyncStatus {
    /// The current chain tip.
    pub tip: L2BlockCommitment,

    /// The previous epoch (ie. epoch most recently completed).
    pub prev_epoch: EpochCommitment,

    /// The finalized epoch, ie what's witnessed on L1.
    pub finalized_epoch: EpochCommitment,
}

impl ChainSyncStatus {
    pub fn cur_epoch(&self) -> u64 {
        self.prev_epoch.epoch() + 1
    }
}

impl ChainSyncStatus {
    pub fn new(
        tip: L2BlockCommitment,
        prev_epoch: EpochCommitment,
        finalized_epoch: EpochCommitment,
    ) -> Self {
        Self {
            tip,
            prev_epoch,
            finalized_epoch,
        }
    }
}

/// Published to the FCM status including chainstate.
#[derive(Clone)]
pub struct ChainSyncStatusUpdate {
    new_status: ChainSyncStatus,
    new_tl_chainstate: Arc<Chainstate>,
}

impl ChainSyncStatusUpdate {
    pub fn new(new_status: ChainSyncStatus, new_tl_chainstate: Arc<Chainstate>) -> Self {
        Self {
            new_status,
            new_tl_chainstate,
        }
    }

    pub fn new_status(&self) -> ChainSyncStatus {
        self.new_status
    }

    pub fn new_tl_chainstate(&self) -> &Arc<Chainstate> {
        &self.new_tl_chainstate
    }

    /// Returns the current epoch.
    pub fn cur_epoch(&self) -> u64 {
        self.new_status().cur_epoch()
    }
}
