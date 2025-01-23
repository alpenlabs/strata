//! High-level chainstate interface.

use std::sync::Arc;

use strata_db::{traits::*, DbResult};
use strata_state::{chain_state::Chainstate, state_op::WriteBatch};
use threadpool::ThreadPool;

use crate::{cache, ops};

pub struct ChainstateManager {
    ops: ops::chainstate::ChainstateOps,
    wb_cache: cache::CacheTable<u64, Option<WriteBatch>>,
}

impl ChainstateManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::chainstate::Context::new(db.chain_state_db().clone()).into_ops(pool);
        let wb_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self { ops, wb_cache }
    }

    // Basic functions that map directly onto database operations.

    /// Writes the genesis state.  This only exists in blocking form because
    /// that's all we need.
    pub fn write_genesis_state(&self, toplevel: Chainstate) -> DbResult<()> {
        self.ops.write_genesis_state_blocking(toplevel)
    }

    pub async fn put_write_batch_async(&self, idx: u64, wb: WriteBatch) -> DbResult<()> {
        self.ops.write_state_update_async(idx, wb).await?;
        self.wb_cache.purge(&idx);
        Ok(())
    }

    pub fn put_write_batch_blocking(&self, idx: u64, wb: WriteBatch) -> DbResult<()> {
        self.ops.write_state_update_blocking(idx, wb)?;
        self.wb_cache.purge(&idx);
        Ok(())
    }

    /// Gets the writes stored for an index.
    pub async fn get_writes_at_async(&self, idx: u64) -> DbResult<Option<WriteBatch>> {
        self.wb_cache
            .get_or_fetch(&idx, || self.ops.get_writes_at_chan(idx))
            .await?
    }

    /// Gets the writes stored for an index.
    pub fn get_writes_at_blocking(&self, idx: u64) -> DbResult<Option<WriteBatch>> {
        self.wb_cache
            .get_or_fetch_blocking(&idx, || self.ops.get_writes_at_blocking(idx))
    }

    pub async fn purge_state_before_async(&self, before_idx: u64) -> DbResult<()> {
        self.ops
            .purge_historical_state_before_async(before_idx)
            .await?;
        self.wb_cache.purge_if(|k| *k < before_idx);
        Ok(())
    }

    pub fn purge_state_before_blocking(&self, before_idx: u64) -> DbResult<()> {
        self.ops
            .purge_historical_state_before_blocking(before_idx)?;
        self.wb_cache.purge_if(|k| *k < before_idx);
        Ok(())
    }

    /// Rolls back writes after a given new tip index, making it the newest tip.
    pub async fn rollback_writes_to_async(&self, new_tip_idx: u64) -> DbResult<()> {
        self.ops.rollback_writes_to_async(new_tip_idx).await?;
        self.wb_cache.purge_if(|k| *k > new_tip_idx);
        Ok(())
    }

    /// Rolls back writes after a given new tip index, making it the newest tip.
    pub fn rollback_writes_to_blocking(&self, new_tip_idx: u64) -> DbResult<()> {
        self.ops.rollback_writes_to_blocking(new_tip_idx)?;
        self.wb_cache.purge_if(|k| *k > new_tip_idx);
        Ok(())
    }

    pub async fn get_first_state_idx_async(&self) -> DbResult<u64> {
        // TODO convert to keep this cached in memory so we don't need both variants
        self.ops.get_earliest_state_idx_async().await
    }

    pub fn get_first_state_idx_blocking(&self) -> DbResult<u64> {
        // TODO convert to keep this cached in memory so we don't need both variants
        self.ops.get_earliest_state_idx_blocking()
    }

    pub async fn get_last_state_idx_async(&self) -> DbResult<u64> {
        // TODO convert to keep this cached in memory so we don't need both variants
        self.ops.get_last_state_idx_async().await
    }

    pub fn get_last_state_idx_blocking(&self) -> DbResult<u64> {
        // TODO convert to keep this cached in memory so we don't need both variants
        self.ops.get_last_state_idx_blocking()
    }

    // Nontrivial functions that aren't just 1:1.

    /// Convenience function just for extracting the toplevel chainstate from
    /// the write batch at an index.
    pub async fn get_toplevel_chainstate_async(&self, idx: u64) -> DbResult<Option<Chainstate>> {
        Ok(self
            .get_writes_at_async(idx)
            .await?
            .map(|wb| wb.into_toplevel()))
    }

    /// Convenience function just for extracting the toplevel chainstate from
    /// the write batch at an index.
    pub fn get_toplevel_chainstate_blocking(&self, idx: u64) -> DbResult<Option<Chainstate>> {
        Ok(self
            .get_writes_at_blocking(idx)?
            .map(|wb| wb.into_toplevel()))
    }
}
