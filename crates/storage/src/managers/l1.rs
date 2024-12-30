use std::sync::Arc;

use strata_db::{traits::Database, DbResult};
use strata_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1TxRef},
};
use strata_state::l1::L1Tx;
use threadpool::ThreadPool;

use crate::{
    cache::{self, CacheTable},
    ops,
};

/// Caching manager of L1 block data
pub struct L1BlockManager {
    ops: ops::l1::L1DataOps,
    manifest_cache: CacheTable<u64, Option<L1BlockManifest>>,
    txs_cache: CacheTable<u64, Option<Vec<L1TxRef>>>,
}

impl L1BlockManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l1::Context::new(db).into_ops(pool);
        let manifest_cache = cache::CacheTable::new(64.try_into().unwrap());
        let txs_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self {
            ops,
            manifest_cache,
            txs_cache,
        }
    }

    pub fn put_block_data(&self, idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        self.ops.put_block_data_blocking(idx, mf, txs)?;
        self.manifest_cache.purge(&idx);
        self.txs_cache.purge(&idx);
        Ok(())
    }

    pub async fn put_block_data_async(
        &self,
        idx: u64,
        mf: L1BlockManifest,
        txs: Vec<L1Tx>,
    ) -> DbResult<()> {
        self.ops.put_block_data_async(idx, mf, txs).await?;
        self.manifest_cache.purge(&idx);
        self.txs_cache.purge(&idx);
        Ok(())
    }

    pub fn revert_to_height(&self, idx: u64) -> DbResult<()> {
        let res = self.ops.revert_to_height_blocking(idx);

        // Purge from cache
        if let Some(tip) = self.ops.get_chain_tip_blocking()? {
            for i in idx..=tip {
                self.manifest_cache.purge(&i);
            }
        }

        res
    }

    pub async fn revert_to_height_async(&self, idx: u64) -> DbResult<()> {
        let res = self.ops.revert_to_height_async(idx).await;

        // Purge from cache
        if let Some(tip) = self.ops.get_chain_tip_blocking()? {
            for i in idx..=tip {
                self.manifest_cache.purge(&i);
            }
        }
        res
    }

    pub fn get_chain_tip(&self) -> DbResult<Option<u64>> {
        self.ops.get_chain_tip_blocking()
    }

    pub async fn get_chain_tip_async(&self) -> DbResult<Option<u64>> {
        self.ops.get_chain_tip_async().await
    }

    pub fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch_blocking(&idx, || self.ops.get_block_manifest_blocking(idx))
    }

    pub async fn get_block_manifest_async(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch(&idx, || self.ops.get_block_manifest_chan(idx))
            .await
    }

    pub fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>> {
        self.ops.get_blockid_range_blocking(start_idx, end_idx)
    }

    pub async fn get_blockid_range_async(
        &self,
        start_idx: u64,
        end_idx: u64,
    ) -> DbResult<Vec<Buf32>> {
        self.ops.get_blockid_range_async(start_idx, end_idx).await
    }

    pub fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch_blocking(&idx, || self.ops.get_block_txs_blocking(idx))
    }

    pub async fn get_block_txs_async(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch(&idx, || self.ops.get_block_txs_chan(idx))
            .await
    }

    pub fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_blocking(tx_ref)
    }

    pub async fn get_tx_async(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_async(tx_ref).await
    }

    pub fn get_txs_from(&self, start_idx: u64) -> DbResult<(Vec<L1Tx>, u64)> {
        self.ops.get_txs_from_blocking(start_idx)
    }

    pub async fn get_txs_from_async(&self, start_idx: u64) -> DbResult<(Vec<L1Tx>, u64)> {
        self.ops.get_txs_from_async(start_idx).await
    }
}
