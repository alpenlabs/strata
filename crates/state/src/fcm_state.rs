//! Descriptors for publishing updated FCM state to other components.

use strata_primitives::l2::L2BlockCommitment;

use crate::epoch::EpochCommitment;

/// Summary of the current FCM state that can be consumed by other components.
pub struct FcmState {
    tip: L2BlockCommitment,
    last_epoch: EpochCommitment,
    finalized_epoch: EpochCommitment,
}

impl FcmState {
    pub fn new(
        l2_block: L2BlockCommitment,
        last_epoch: EpochCommitment,
        finalized_epoch: EpochCommitment,
    ) -> Self {
        Self {
            tip: l2_block,
            last_epoch,
            finalized_epoch,
        }
    }

    /// Currently accepted tip block.
    pub fn tip(&self) -> L2BlockCommitment {
        self.tip
    }

    /// Currently accepted last epoch tip block.
    pub fn last_epoch(&self) -> EpochCommitment {
        self.last_epoch
    }

    /// Epoch that the chain and we accept as final.
    pub fn finalized_epoch(&self) -> EpochCommitment {
        self.finalized_epoch
    }
}
