use std::sync::Arc;

use alpen_express_db::errors::DbError;
use alpen_express_db::DbResult;
use threadpool::ThreadPool;

use alpen_express_db::traits::Database;
use alpen_express_state::{block::L2BlockBundle, header::L2Header, id::L2BlockId};

use crate::cache;
use crate::ops;

pub struct L2BlockManager {
    ops: ops::l2::L2DataOps,
    block_cache: cache::CacheTable<L2BlockId, Option<L2BlockBundle>>,
}

impl L2BlockManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        // TODO this still feels like more ceremony than we need, should improve macro
        let ctx = ops::l2::Context::new(db);
        let ops = ops::l2::L2DataOps::new(pool, Arc::new(ctx));
        let block_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self { ops, block_cache }
    }

    /// Puts a block in the database, purging cache entry.
    pub async fn put_block_async(&self, bundle: L2BlockBundle) -> DbResult<()> {
        let id = bundle.block().header().get_blockid();
        self.ops.put_block_async(bundle).await?;
        self.block_cache.purge_async(&id).await;
        Ok(())
    }

    /// Puts in a block in the database, purging cache entry.
    pub fn put_block_blocking(&self, bundle: L2BlockBundle) -> DbResult<()> {
        let id = bundle.block().header().get_blockid();
        self.ops.put_block_blocking(bundle)?;
        self.block_cache.purge_blocking(&id);
        Ok(())
    }

    /// Gets a block either in the cache or from the underlying database.
    pub async fn get_block_async(&self, k: &L2BlockId) -> Result<Option<L2BlockBundle>, DbError> {
        self.block_cache
            .get_or_fetch_async(k, || self.ops.get_block_chan(*k))
            .await
    }

    /// Gets a block either in the cache or from the underlying database.
    pub fn get_block_blocking(&self, k: &L2BlockId) -> Result<Option<L2BlockBundle>, DbError> {
        self.block_cache
            .get_or_fetch_blocking(k, || self.ops.get_block_blocking(*k))
    }
}
