use std::sync::Arc;

use alpen_express_db::{traits::Database, types::CheckpointEntry, DbResult};
use threadpool::ThreadPool;

use crate::{cache, ops};

pub struct CheckpointManager {
    ops: ops::checkpoint::CheckpointDataOps,
    checkpoint_cache: cache::CacheTable<u64, Option<CheckpointEntry>>,
}

impl CheckpointManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::checkpoint::Context::new(db).into_ops(pool);
        let checkpoint_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self {
            ops,
            checkpoint_cache,
        }
    }

    pub async fn put_checkpoint(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.ops.put_batch_checkpoint_async(idx, entry).await?;
        self.checkpoint_cache.purge_async(&idx).await;
        Ok(())
    }

    pub fn put_checkpoint_blocking(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.ops.put_batch_checkpoint_blocking(idx, entry)?;
        self.checkpoint_cache.purge_blocking(&idx);
        Ok(())
    }

    pub async fn get_checkpoint(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.checkpoint_cache
            .get_or_fetch_async(&idx, || self.ops.get_batch_checkpoint_chan(idx))
            .await
    }

    pub fn get_checkpoint_blocking(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.checkpoint_cache
            .get_or_fetch_blocking(&idx, || self.ops.get_batch_checkpoint_blocking(idx))
    }
}
