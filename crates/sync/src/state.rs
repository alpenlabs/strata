use std::cmp::max;

use strata_consensus_logic::unfinalized_tracker::UnfinalizedBlockTracker;
use strata_state::{
    client_state::SyncState,
    header::{L2Header, SignedL2BlockHeader},
    id::L2BlockId,
};
use strata_storage::L2BlockManager;
use tracing::debug;

use crate::L2SyncError;

pub struct L2SyncState {
    finalized_height: u64,
    /// Height of highest unfinalized block in tracker
    tip_height: u64,
    tracker: UnfinalizedBlockTracker,
}

impl L2SyncState {
    pub(crate) fn attach_block(
        &mut self,
        block_header: &SignedL2BlockHeader,
    ) -> Result<(), L2SyncError> {
        self.tracker
            .attach_block(block_header.get_blockid(), block_header)?;
        let block_height = block_header.blockidx();
        self.tip_height = max(self.tip_height, block_height);
        Ok(())
    }

    pub(crate) fn update_finalized_tip(
        &mut self,
        block_id: &L2BlockId,
        block_height: u64,
    ) -> Result<(), L2SyncError> {
        self.tracker.update_finalized_tip(block_id)?;
        self.finalized_height = block_height;
        Ok(())
    }

    pub(crate) fn has_block(&self, block_id: &L2BlockId) -> bool {
        self.tracker.is_seen_block(block_id)
    }

    pub(crate) fn finalized_height(&self) -> u64 {
        self.finalized_height
    }

    pub(crate) fn finalized_blockid(&self) -> &L2BlockId {
        self.tracker.finalized_tip()
    }

    pub(crate) fn tip_height(&self) -> u64 {
        self.tip_height
    }
}

pub fn initialize_from_db(
    sync: &SyncState,
    l2_block_manager: &L2BlockManager,
) -> Result<L2SyncState, L2SyncError> {
    let finalized_blockid = sync.finalized_blkid();
    let finalized_block = l2_block_manager.get_block_data_blocking(finalized_blockid)?;
    let Some(finalized_block) = finalized_block else {
        return Err(L2SyncError::MissingBlock(*finalized_blockid));
    };
    let finalized_height = finalized_block.header().blockidx();
    let tip_height = sync.tip_height();

    debug!(finalized_blockid = ?finalized_blockid, finalized_height = finalized_height, tip_height = tip_height, "init unfinalized blocks");

    let mut tracker = UnfinalizedBlockTracker::new_empty(*finalized_blockid);
    tracker
        .load_unfinalized_blocks(finalized_height, tip_height, l2_block_manager)
        .map_err(|err| L2SyncError::LoadUnfinalizedFailed(err.to_string()))?;

    let state = L2SyncState {
        finalized_height,
        tip_height,
        tracker,
    };

    Ok(state)
}
