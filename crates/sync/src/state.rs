use strata_consensus_logic::unfinalized_tracker::UnfinalizedBlockTracker;
use strata_primitives::{epoch::EpochCommitment, l2::L2BlockCommitment};
use strata_state::{
    client_state::ClientState,
    header::{L2Header, SignedL2BlockHeader},
    id::L2BlockId,
};
use strata_storage::NodeStorage;
use tracing::{debug, warn};

use crate::L2SyncError;

pub struct L2SyncState {
    /// Height of highest unfinalized block in tracker
    tip_block: L2BlockCommitment,

    // TODO make this just subscribe to FCM tip updates and go from there?
    tracker: UnfinalizedBlockTracker,
}

impl L2SyncState {
    pub(crate) fn attach_block(
        &mut self,
        block_header: &SignedL2BlockHeader,
    ) -> Result<(), L2SyncError> {
        self.tracker
            .attach_block(block_header.get_blockid(), block_header)?;

        // FIXME this isn't quite right, we should be following the fork choice manager
        self.tip_block = self
            .tracker
            .chain_tip_blocks_iter()
            .max_by_key(|bc| bc.slot())
            .expect("sync: picking new tip");

        Ok(())
    }

    pub(crate) fn update_finalized_tip(
        &mut self,
        epoch: EpochCommitment,
    ) -> Result<(), L2SyncError> {
        self.tracker.update_finalized_epoch(&epoch)?;
        Ok(())
    }

    pub(crate) fn has_block(&self, block_id: &L2BlockId) -> bool {
        self.tracker.is_seen_block(block_id)
    }

    // TODO rename to slot
    pub(crate) fn finalized_height(&self) -> u64 {
        self.tracker.finalized_epoch().last_slot()
    }

    pub(crate) fn finalized_blockid(&self) -> &L2BlockId {
        self.tracker.finalized_epoch().last_blkid()
    }

    // TODO rename to slot
    pub(crate) fn tip_height(&self) -> u64 {
        self.tip_block.slot()
    }
}

pub(crate) async fn initialize_from_db(
    cstate: &ClientState,
    storage: &NodeStorage,
) -> Result<L2SyncState, L2SyncError> {
    let l2man = storage.l2().as_ref();
    let finalized_epoch = match cstate.get_apparent_finalized_epoch() {
        Some(epoch) => epoch,
        None => {
            // TODO handle this in some more structured way
            warn!("no finalized block yet, sync starting from dummy genesis");
            let blocks = l2man.get_blocks_at_height_blocking(0)?;
            let genesis_blkid = blocks[0];
            EpochCommitment::new(0, 0, genesis_blkid)
        }
    };

    let finalized_blkid = *finalized_epoch.last_blkid();
    let finalized_slot = finalized_epoch.last_slot();

    let finalized_block = l2man.get_block_data_blocking(&finalized_blkid)?;

    // Should we remove this since we don't do anything with it anymore?  It
    // does serve as a sanity check so that we don't start on a block we don't
    // actually have.
    let Some(_) = finalized_block else {
        return Err(L2SyncError::MissingBlock(*finalized_epoch.last_blkid()));
    };

    debug!(?finalized_blkid, %finalized_slot, "loading unfinalized blocks");

    let mut tracker = UnfinalizedBlockTracker::new_empty(finalized_epoch);
    tracker
        .load_unfinalized_blocks(l2man)
        .map_err(|err| L2SyncError::LoadUnfinalizedFailed(err.to_string()))?;

    let tip_block = tracker
        .chain_tip_blocks_iter()
        .max_by_key(|bc| bc.slot())
        .expect("sync: missing init chain tip");

    let state = L2SyncState { tip_block, tracker };

    Ok(state)
}
