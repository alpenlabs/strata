use std::{
    cmp::{max, min},
    sync::Arc,
};

use alpen_express_consensus_logic::{
    errors::ChainTipError, message::ForkChoiceMessage, sync_manager::SyncManager,
    unfinalized_tracker::UnfinalizedBlockTracker,
};
use alpen_express_db::DbError;
use alpen_express_rpc_types::NodeSyncStatus;
use alpen_express_state::{
    block::L2BlockBundle,
    client_state::SyncState,
    header::{L2Header, SignedL2BlockHeader},
    id::L2BlockId,
};
use express_storage::L2BlockManager;
use tracing::{debug, error, warn};

use crate::{SyncPeer, SyncPeerError};

const SYNC_BATCH_SIZE: u64 = 10;

#[derive(Debug, thiserror::Error)]
pub enum L2SyncError {
    #[error("block not found")]
    NotFound,
    #[error("wrong fork")]
    WrongFork,
    #[error("missing parent block")]
    MissingParent,
    #[error("peer error: {0}")]
    PeerError(#[from] SyncPeerError),
    #[error("db error: {0}")]
    DbError(#[from] DbError),
    #[error("chain tip error: {0}")]
    ChainTipError(#[from] ChainTipError),
    #[error("other: {0}")]
    Other(String),
}

struct UnfinalizedBlocks {
    finalized_height: u64,
    tip_height: u64,
    tracker: UnfinalizedBlockTracker,
}

impl UnfinalizedBlocks {
    async fn new_from_db(
        sync: SyncState,
        l2_block_manager: &L2BlockManager,
    ) -> Result<Self, L2SyncError> {
        let finalized_blockid = sync.finalized_blkid();
        let finalized_block = l2_block_manager.get_block_async(finalized_blockid).await?;
        let Some(finalized_block) = finalized_block else {
            return Err(L2SyncError::NotFound);
        };
        let finalized_height = finalized_block.header().blockidx();
        let tip_height = sync.chain_tip_height();

        debug!(finalized_blockid = ?finalized_blockid, finalized_height = finalized_height, tip_height = tip_height, "init unfinalized blocks");

        let mut tracker = UnfinalizedBlockTracker::new_empty(*finalized_blockid);
        tracker
            .load_unfinalized_blocks_async(finalized_height, tip_height, l2_block_manager)
            .await
            .map_err(|err| L2SyncError::Other(err.to_string()))?;

        let unfinalized_blocks = Self {
            finalized_height,
            tip_height,
            tracker,
        };

        Ok(unfinalized_blocks)
    }

    fn attach_block(&mut self, block_header: &SignedL2BlockHeader) -> Result<(), L2SyncError> {
        self.tracker
            .attach_block(block_header.get_blockid(), block_header)?;
        let block_height = block_header.blockidx();
        self.tip_height = max(self.tip_height, block_height);
        Ok(())
    }

    fn update_finalized_tip(
        &mut self,
        block_id: &L2BlockId,
        block_height: u64,
    ) -> Result<(), L2SyncError> {
        self.tracker.update_finalized_tip(block_id)?;
        self.finalized_height = block_height;
        Ok(())
    }

    fn has_block(&self, block_id: &L2BlockId) -> bool {
        self.tracker.is_seen_block(block_id)
    }
}

pub struct L2SyncManager<T: SyncPeer> {
    peer: T,
    l2_manager: Arc<L2BlockManager>,
    sync_man: Arc<SyncManager>,
}

impl<T: SyncPeer> L2SyncManager<T> {
    pub fn new(peer: T, l2_manager: Arc<L2BlockManager>, sync_man: Arc<SyncManager>) -> Self {
        Self {
            peer,
            l2_manager,
            sync_man,
        }
    }

    async fn wait_for_csm_ready(&self) -> SyncState {
        let mut client_update_notif = self.sync_man.create_cstate_subscription();

        loop {
            let Ok(update) = client_update_notif.recv().await else {
                continue;
            };
            let state = update.new_state();
            if state.is_chain_active() && state.sync().is_some() {
                return state.sync().unwrap().clone();
            }
        }
    }

    pub async fn run(self) -> Result<(), L2SyncError> {
        debug!("waiting for CSM to become ready");
        let sync_state = self.wait_for_csm_ready().await;

        debug!(sync_state = ?sync_state, "CSM is ready");

        let unfinalized_blocks =
            UnfinalizedBlocks::new_from_db(sync_state, &self.l2_manager).await?;

        let mut manager = L2SyncManagerInitialized {
            peer: self.peer,
            unfinalized_blocks,
            l2_manager: self.l2_manager,
            sync_man: self.sync_man,
        };

        manager.run().await
    }
}

pub struct L2SyncManagerInitialized<T: SyncPeer> {
    peer: T,
    unfinalized_blocks: UnfinalizedBlocks,
    l2_manager: Arc<L2BlockManager>,
    sync_man: Arc<SyncManager>,
}

impl<T: SyncPeer> L2SyncManagerInitialized<T> {
    fn tip_height(&self) -> u64 {
        self.unfinalized_blocks.tip_height
    }

    fn finalized_height(&self) -> u64 {
        self.unfinalized_blocks.finalized_height
    }

    async fn get_peer_status(&self) -> Result<NodeSyncStatus, SyncPeerError> {
        self.peer.fetch_sync_status().await
    }

    #[allow(unused)]
    async fn sync_block(&mut self, block_id: &L2BlockId) -> Result<(), L2SyncError> {
        let Some(block) = self.peer.fetch_block_by_id(block_id).await? else {
            return Err(L2SyncError::NotFound);
        };

        self.on_new_block(block).await?;

        Ok(())
    }

    #[allow(unused)]
    async fn sync_blocks_by_height(&mut self, height: u64) -> Result<(), L2SyncError> {
        debug!(height = height, "syncing blocks by height");
        let blocks = self.peer.fetch_blocks_by_height(height).await?;

        debug!(height = height, "received {} blocks", blocks.len());

        for block in blocks {
            self.on_new_block(block).await?;
        }

        Ok(())
    }

    async fn sync_blocks_by_range(
        &mut self,
        start_height: u64,
        end_height: u64,
    ) -> Result<(), L2SyncError> {
        debug!(
            start_height = start_height,
            end_height = end_height,
            "syncing blocks by range"
        );
        let blocks = self
            .peer
            .fetch_blocks_by_range(start_height, end_height)
            .await?;

        debug!("received {} blocks", blocks.len());

        for block in blocks {
            self.on_new_block(block).await?;
        }

        Ok(())
    }

    /// Process a new block received from the peer.
    ///
    /// The block is added to the unfinalized chain and the corresponding
    /// messages are submitted to the sync manager.
    ///
    /// If the parent block is missing, it will be fetched recursively
    /// until we reach a known block in our unfinalized chain.
    async fn on_new_block(&mut self, block: L2BlockBundle) -> Result<(), L2SyncError> {
        let mut block = block;
        let mut fetched_blocks = vec![];

        loop {
            let block_id = block.header().get_blockid();
            debug!(block_id = ?block_id, height = block.header().blockidx(), "received new block");

            if self.unfinalized_blocks.has_block(&block_id) {
                warn!(block_id = ?block_id, "block already known");
                return Ok(());
            }

            let height = block.header().blockidx();
            if height <= self.finalized_height() {
                // saw block on different fork than one we're finalized on
                // log error and ignore received blocks
                error!(
                    "block at height {height} is already finalized; cannot overwrite: {block_id}"
                );
                return Err(L2SyncError::WrongFork);
            }

            let parent_block_id = block.header().parent();

            fetched_blocks.push(block.clone());

            if self.unfinalized_blocks.has_block(parent_block_id) {
                break;
            }

            // parent block does not exist in our unfinalized chain
            // try to fetch it and continue
            let Some(parent_block) = self.peer.fetch_block_by_id(parent_block_id).await? else {
                // block not found
                error!("parent block {parent_block_id} not found");
                return Err(L2SyncError::NotFound);
            };

            block = parent_block;
        }

        // send ForkChoiceMessage::NewBlock for all pending blocks in correct order
        while let Some(block) = fetched_blocks.pop() {
            self.unfinalized_blocks.attach_block(block.header())?;
            self.l2_manager.put_block_async(block.clone()).await?;
            self.sync_man
                .submit_chain_tip_msg_async(ForkChoiceMessage::NewBlock(
                    block.header().get_blockid(),
                ))
                .await;
        }
        Ok(())
    }

    async fn on_block_finalized(
        &mut self,
        finalized_blockid: &L2BlockId,
        finalized_height: u64,
    ) -> Result<(), L2SyncError> {
        if self.unfinalized_blocks.tracker.finalized_tip() == finalized_blockid {
            return Ok(());
        }

        if !self
            .unfinalized_blocks
            .tracker
            .is_seen_block(finalized_blockid)
        {
            return Err(L2SyncError::Other("invalid finalized block".to_string()));
        };

        self.unfinalized_blocks
            .update_finalized_tip(finalized_blockid, finalized_height)?;

        Ok(())
    }

    async fn run(&mut self) -> Result<(), L2SyncError> {
        let mut client_update_notif = self.sync_man.create_cstate_subscription();
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            tokio::select! {
                client_update = client_update_notif.recv() => {
                    // on receiving new client update, update own finalized state
                    let Ok(update) = client_update else {
                        continue;
                    };
                    let Some(sync) = update.new_state().sync() else {
                        continue;
                    };

                    let finalized_blockid = sync.finalized_blkid();
                    let Ok(Some(finalized_block)) = self.l2_manager.get_block_async(finalized_blockid).await else {
                        error!("missing finalized block {}", finalized_blockid);
                        continue;
                    };
                    let finalized_height = finalized_block.header().blockidx();
                    if let Err(err) = self.on_block_finalized(finalized_blockid, finalized_height).await {
                        error!("failed to finalize block {}: {err}", sync.finalized_blkid());
                    }
                }
                _ = interval.tick() => {
                    // every fixed interval, try to sync with latest state of peer
                    let Ok(status) = self.get_peer_status().await else {
                        warn!("failed to get peer status");
                        continue;
                    };

                    if self.unfinalized_blocks.tracker.is_seen_block(&status.tip_block_id) {
                        // in sync with peer
                        continue;
                    }

                    debug!("syncing to height {}; current height {}", status.tip_height, self.tip_height());

                    // actually height, but almost equivalent
                    let sync_block_count = min(status.tip_height - self.tip_height(), SYNC_BATCH_SIZE);
                    let start_height = self.tip_height() + 1;
                    let end_height = self.tip_height() + sync_block_count;

                    if let Err(err) = self.sync_blocks_by_range(start_height, end_height).await {
                        error!("failed to sync blocks {start_height}-{end_height}: {err}");
                    }
                }
                // TODO: subscribe to new blocks on peer instead of polling
            }
        }
    }
}
