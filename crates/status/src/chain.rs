//! Container for chain status.

use std::sync::Arc;

use strata_primitives::{epoch::EpochCommitment, l2::L2BlockCommitment, prelude::*};
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

    /// The last L1 block we've observed.
    pub safe_l1: L1BlockCommitment,
}

impl ChainSyncStatus {
    pub fn tip_slot(&self) -> u64 {
        self.tip.slot()
    }

    pub fn tip_blkid(&self) -> &L2BlockId {
        self.tip.blkid()
    }

    pub fn finalized_blkid(&self) -> &L2BlockId {
        self.finalized_epoch.last_blkid()
    }

    pub fn cur_epoch(&self) -> u64 {
        self.prev_epoch.epoch() + 1
    }
}

impl ChainSyncStatus {
    pub fn new(
        tip: L2BlockCommitment,
        prev_epoch: EpochCommitment,
        finalized_epoch: EpochCommitment,
        safe_l1: L1BlockCommitment,
    ) -> Self {
        Self {
            tip,
            prev_epoch,
            finalized_epoch,
            safe_l1,
        }
    }

    /// Transitional function for as long as we can construct an instance of this
    /// type from a chainstate.
    pub fn from_transitional(chs: &Chainstate) -> Self {
        let tip = L2BlockCommitment::new(chs.chain_tip_slot(), *chs.chain_tip_blkid());
        Self::new(
            tip,
            *chs.prev_epoch(),
            *chs.finalized_epoch(),
            chs.l1_view().get_safe_block(),
        )
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

    /// Transitional function for directly constructing the status update from
    /// the full chainstate.
    pub fn new_transitional(new_tl_chainstate: Arc<Chainstate>) -> Self {
        let css = ChainSyncStatus::from_transitional(new_tl_chainstate.as_ref());
        Self::new(css, new_tl_chainstate)
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

/// Status of different services
/// Currently only fcm, others can be added as per need.
#[derive(Clone)]
pub struct ServiceInitStatus {
    fcm: bool,
}

impl ServiceInitStatus {
    pub fn new_uninitialized() -> Self {
        Self { fcm: false }
    }

    pub fn is_fcm_initialized(&self) -> bool {
        self.fcm
    }

    pub fn set_fcm_initialized(&mut self) {
        self.fcm = true;
    }
}
