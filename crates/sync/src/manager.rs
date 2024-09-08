use std::sync::Arc;

use alpen_express_consensus_logic::{message::ForkChoiceMessage, sync_manager::SyncManager};
use alpen_express_db::{
    traits::{ClientStateProvider, Database},
    DbError,
};
use alpen_express_rpc_types::NodeSyncStatus;
use alpen_express_state::{block::L2BlockBundle, header::L2Header, id::L2BlockId};
use express_storage::L2BlockManager;
use indexmap::IndexMap;
use tracing::{error, warn};

use crate::{SyncPeer, SyncPeerError};

#[derive(Debug, thiserror::Error)]
pub enum L2SyncError {
    #[error("block not found")]
    NotFound,
    #[error("missing parent block")]
    MissingParent,
    #[error("peer error: {0}")]
    PeerError(#[from] SyncPeerError),
    #[error("db error: {0}")]
    DbError(#[from] DbError),
    #[error("other: {0}")]
    Other(String),
}

pub struct L2SyncManager<T: SyncPeer> {
    peer: T,
    // includes blocks from finalized height to tip, inclusive
    unfinalized_blocks: IndexMap<L2BlockId, u64>,
    // temporary storage for blocks whose parent is not yet fetched
    // need this because fcm needs blocks to be in order
    fetched_blocks: Vec<L2BlockId>,
    finalized_height: u64,
    l2_manager: Arc<L2BlockManager>,
    sync_man: Arc<SyncManager>,
}

impl<T: SyncPeer> L2SyncManager<T> {
    pub fn new(
        peer: T,
        l2_manager: Arc<L2BlockManager>,
        sync_man: Arc<SyncManager>,
        database: Arc<impl Database>,
    ) -> Self {
        let (finalized_blkid, finalized_height) = get_finalized_block_info(&l2_manager, database)
            .unwrap_or(get_genesis_block_info(&l2_manager));

        let unfinalized_blocks = IndexMap::from_iter(vec![(finalized_blkid, finalized_height)]);
        let fetched_blocks = Vec::new();

        Self {
            peer,
            unfinalized_blocks,
            fetched_blocks,
            finalized_height,
            l2_manager,
            sync_man,
        }
    }

    fn tip_block_id(&self) -> &L2BlockId {
        self.unfinalized_blocks.last().expect("non empty").0
    }

    #[allow(unused)]
    fn tip_height(&self) -> u64 {
        *self.unfinalized_blocks.last().expect("non empty").1
    }

    async fn get_peer_status(&self) -> Result<NodeSyncStatus, SyncPeerError> {
        self.peer.fetch_sync_status().await
    }

    async fn sync_block(&mut self, block_id: &L2BlockId) -> Result<(), L2SyncError> {
        let Some(block) = self.peer.fetch_block_by_id(block_id).await? else {
            return Err(L2SyncError::NotFound);
        };

        self.on_new_block(block).await?;

        Ok(())
    }

    async fn on_new_block(&mut self, block: L2BlockBundle) -> Result<(), L2SyncError> {
        let block_id = block.header().get_blockid();

        if self.unfinalized_blocks.contains_key(&block_id) {
            warn!("block already known: {block_id}");
            return Ok(());
        }

        let height = block.header().blockidx();
        if height <= self.finalized_height {
            // saw block on different fork than one we're finalized on
            // log error and ignore received blocks
            error!("block at height {height} is already finalized; cannot overwrite: {block_id}");
            self.fetched_blocks.clear();
            // remove from db ?
            return Ok(());
        }

        let parent_block_id = block.header().parent();

        // insert into db
        self.l2_manager.put_block_async(block.clone()).await?;

        if self.unfinalized_blocks.contains_key(parent_block_id) {
            // parent block exists in our unfinalized chain. Proceeding with sync
            self.fetched_blocks.push(block_id);
            let mut height = height;
            // send ForkChoiceMessage::NewBlock for all pending blocks in correct order
            while let Some(block_id) = self.fetched_blocks.pop() {
                self.unfinalized_blocks.insert(block_id, height);
                height += 1;
                self.sync_man
                    .submit_chain_tip_msg(ForkChoiceMessage::NewBlock(block_id));
            }
        } else {
            // parent block does not exist
            // add to fetching list to process later
            self.fetched_blocks.push(block_id);
            // fetch missing parent block
            match Box::pin(self.sync_block(parent_block_id)).await {
                Err(L2SyncError::NotFound) => {
                    // invalid fork
                    error!("parent block {parent_block_id} in chain not found");
                    self.fetched_blocks.clear();
                    // remove from db ?
                    return Err(L2SyncError::MissingParent);
                }
                res => {
                    res?;
                }
            }
        }
        Ok(())
    }

    async fn on_block_finalized(&mut self, block_id: &L2BlockId) -> Result<(), L2SyncError> {
        let Some(finalized_idx) = self.unfinalized_blocks.get_index_of(block_id) else {
            return Err(L2SyncError::Other("invalid finalized block".to_string()));
        };

        // remove all blocks before finalized height
        self.unfinalized_blocks = IndexMap::from_iter(
            self.unfinalized_blocks
                .clone()
                .into_iter()
                .skip(finalized_idx),
        );

        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), L2SyncError> {
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
                    if let Err(err) = self.on_block_finalized(sync.finalized_blkid()).await {
                        error!("failed to finalize block {}: {err}", sync.finalized_blkid());
                    }
                }
                _ = interval.tick() => {
                    // every fixed interval, try to sync with latest state of peer
                    let Ok(status) = self.get_peer_status().await else {
                        warn!("failed to get peer status");
                        continue;
                    };

                    if &status.tip_block_id == self.tip_block_id() {
                        // in sync with peer
                        continue;
                    }

                    // TODO: if status.tip_height - self.tip_height > threshold, sync blocks sequentially

                    if let Err(err) = self.sync_block(&status.tip_block_id).await {
                        error!("failed to sync block {}: {err}", status.tip_block_id);
                    }
                }
                // TODO: subscribe to new blocks
            }
        }
    }
}

fn get_finalized_block_info(
    l2_manager: &L2BlockManager,
    database: Arc<impl Database>,
) -> Option<(L2BlockId, u64)> {
    let finalized_blkid = database
        .client_state_provider()
        .get_last_checkpoint_idx()
        .and_then(|idx| database.client_state_provider().get_state_checkpoint(idx))
        .map(|state| {
            state
                .and_then(|s| s.sync().cloned())
                .map(|s| *s.finalized_blkid())
        })
        .ok()
        .flatten()?;

    match l2_manager.get_block_blocking(&finalized_blkid) {
        Ok(Some(block)) => Some((finalized_blkid, block.header().blockidx())),
        Ok(None) => {
            error!("finalized block {finalized_blkid} not found");
            None
        }
        Err(e) => {
            error!("failed to get finalized block {finalized_blkid}: {e}");
            None
        }
    }
}

fn get_genesis_block_info(l2_manager: &L2BlockManager) -> (L2BlockId, u64) {
    let genesis_block_id = l2_manager
        .get_blocks_at_height_blocking(0)
        .expect("genesis block at height 0 exists")
        .first()
        .cloned()
        .expect("genesis block exists");

    (genesis_block_id, 0)
}
