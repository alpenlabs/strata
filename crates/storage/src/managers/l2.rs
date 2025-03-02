use std::sync::Arc;

use strata_db::{
    traits::{BlockStatus, Database},
    DbResult,
};
use strata_state::{block::L2BlockBundle, header::L2Header, id::L2BlockId};
use threadpool::ThreadPool;
use tokio::sync::broadcast;
use tracing::warn;

use crate::{cache, ops};

/// Caching manager of L2 blocks in the block database.
pub struct L2BlockManager {
    ops: ops::l2::L2DataOps,
    block_cache: cache::CacheTable<L2BlockId, Option<L2BlockBundle>>,
    /// Broadcaster which broadcasts about new l2_block update in db.
    l2_blk_tx: broadcast::Sender<(u64, L2BlockId)>,
}

impl L2BlockManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l2::Context::new(db.l2_db().clone()).into_ops(pool);
        let block_cache = cache::CacheTable::new(64.try_into().unwrap());
        let (l2_blk_tx, _) = broadcast::channel::<(u64, L2BlockId)>(1024);
        Self {
            ops,
            block_cache,
            l2_blk_tx,
        }
    }

    /// Puts a block in the database, purging cache entry.
    pub async fn put_block_data_async(&self, bundle: L2BlockBundle) -> DbResult<()> {
        let header = bundle.block().header().clone();
        let id = header.get_blockid();
        self.ops.put_block_data_async(bundle).await?;
        self.block_cache.purge(&id);

        // Broadcast
        if let Err(e) = self.l2_blk_tx.send((header.blockidx(), id)) {
            warn!(?e, "No active listeners for l2 tx update");
        }
        Ok(())
    }

    /// Puts in a block in the database, purging cache entry.
    pub fn put_block_data_blocking(&self, bundle: L2BlockBundle) -> DbResult<()> {
        let header = bundle.block().header().clone();
        let id = header.get_blockid();
        self.ops.put_block_data_blocking(bundle)?;
        self.block_cache.purge(&id);

        // Broadcast
        if let Err(e) = self.l2_blk_tx.send((header.blockidx(), id)) {
            warn!(?e, "No active listeners for l2 tx update");
        }
        Ok(())
    }

    /// Subscribe to block updates
    pub fn subscribe_to_block_updates(&self) -> broadcast::Receiver<(u64, L2BlockId)> {
        self.l2_blk_tx.subscribe()
    }

    /// Gets a block either in the cache or from the underlying database.
    pub async fn get_block_data_async(&self, id: &L2BlockId) -> DbResult<Option<L2BlockBundle>> {
        self.block_cache
            .get_or_fetch(id, || self.ops.get_block_data_chan(*id))
            .await
    }

    /// Gets a block either in the cache or from the underlying database.
    pub fn get_block_data_blocking(&self, id: &L2BlockId) -> DbResult<Option<L2BlockBundle>> {
        self.block_cache
            .get_or_fetch_blocking(id, || self.ops.get_block_data_blocking(*id))
    }

    /// Gets the block at a height.  Async.
    pub async fn get_blocks_at_height_async(&self, h: u64) -> DbResult<Vec<L2BlockId>> {
        self.ops.get_blocks_at_height_async(h).await
    }

    /// Gets the block at a height.  Blocking.
    pub fn get_blocks_at_height_blocking(&self, h: u64) -> DbResult<Vec<L2BlockId>> {
        self.ops.get_blocks_at_height_blocking(h)
    }

    /// Gets the block's verification status.  Async.
    pub async fn get_block_status_async(&self, id: &L2BlockId) -> DbResult<Option<BlockStatus>> {
        self.ops.get_block_status_async(*id).await
    }

    /// Gets the block's verification status.  Blocking.
    pub fn get_block_status_blocking(&self, id: &L2BlockId) -> DbResult<Option<BlockStatus>> {
        self.ops.get_block_status_blocking(*id)
    }

    /// Sets the block's verification status.  Async.
    pub async fn set_block_status_async(
        &self,
        id: &L2BlockId,
        status: BlockStatus,
    ) -> DbResult<()> {
        self.ops.set_block_status_async(*id, status).await
    }

    /// Sets the block's verification status.  Blocking.
    pub fn set_block_status_blocking(&self, id: &L2BlockId, status: BlockStatus) -> DbResult<()> {
        self.ops.set_block_status_blocking(*id, status)
    }
}
