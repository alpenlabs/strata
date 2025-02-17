use std::sync::Arc;

use strata_db::{traits::L1Database, DbResult};
use strata_primitives::l1::{L1BlockManifest, L1TxRef};
use strata_state::l1::{L1BlockId, L1Tx};
use threadpool::ThreadPool;

use crate::{
    cache::{self, CacheTable},
    ops,
};

/// Caching manager of L1 block data
pub struct L1BlockManager {
    ops: ops::l1::L1DataOps,
    manifest_cache: CacheTable<L1BlockId, Option<L1BlockManifest>>,
    txs_cache: CacheTable<L1BlockId, Option<Vec<L1TxRef>>>,
    blockheight_cache: CacheTable<u64, Option<L1BlockId>>,
}

impl L1BlockManager {
    pub fn new<D: L1Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l1::Context::new(db).into_ops(pool);
        let manifest_cache = cache::CacheTable::new(64.try_into().unwrap());
        let txs_cache = cache::CacheTable::new(64.try_into().unwrap());
        let blockheight_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self {
            ops,
            manifest_cache,
            txs_cache,
            blockheight_cache,
        }
    }

    pub fn put_block_data(&self, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        let blockid = mf.block_hash();
        self.manifest_cache.purge(&blockid);
        self.txs_cache.purge(&blockid);
        self.ops.put_block_data_blocking(mf, txs)
    }

    pub async fn put_block_data_async(&self, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        let blockid = mf.block_hash();
        self.manifest_cache.purge(&blockid);
        self.txs_cache.purge(&blockid);
        self.ops.put_block_data_async(mf, txs).await
    }

    pub fn add_to_canonical_chain(&self, height: u64, blockid: &L1BlockId) -> DbResult<()> {
        self.ops.add_to_canonical_chain_blocking(height, *blockid)
    }

    pub async fn add_to_canonical_chain_async(
        &self,
        height: u64,
        blockid: &L1BlockId,
    ) -> DbResult<()> {
        self.ops
            .add_to_canonical_chain_async(height, *blockid)
            .await
    }

    pub fn revert_canonical_chain(&self, idx: u64) -> DbResult<()> {
        if let Some((tip, _)) = self.ops.get_chain_tip_blocking()? {
            for i in idx + 1..=tip {
                self.blockheight_cache.purge(&i);
            }
        }
        self.ops.revert_canonical_chain_blocking(idx)
    }

    pub async fn revert_canonical_chain_async(&self, idx: u64) -> DbResult<()> {
        if let Some((tip, _)) = self.ops.get_chain_tip_async().await? {
            for i in idx + 1..=tip {
                self.blockheight_cache.purge(&i);
            }
        }

        self.ops.revert_canonical_chain_async(idx).await
    }

    pub fn get_chain_tip(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.ops.get_chain_tip_blocking()
    }

    pub async fn get_chain_tip_async(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.ops.get_chain_tip_async().await
    }

    pub fn get_chain_tip_height(&self) -> DbResult<Option<u64>> {
        Ok(self.get_chain_tip()?.map(|(height, _)| height))
    }

    pub async fn get_chain_tip_height_async(&self) -> DbResult<Option<u64>> {
        Ok(self.get_chain_tip_async().await?.map(|(height, _)| height))
    }

    pub fn get_block_manifest(&self, blockid: &L1BlockId) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch_blocking(blockid, || self.ops.get_block_manifest_blocking(*blockid))
    }

    pub async fn get_block_manifest_async(
        &self,
        blockid: &L1BlockId,
    ) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch(blockid, || self.ops.get_block_manifest_chan(*blockid))
            .await
    }

    pub fn get_block_manifest_at_height(&self, height: u64) -> DbResult<Option<L1BlockManifest>> {
        let Some(blockid) = self.get_canonical_blockid(height)? else {
            return Ok(None);
        };

        self.get_block_manifest(&blockid)
    }

    pub async fn get_block_manifest_at_height_async(
        &self,
        height: u64,
    ) -> DbResult<Option<L1BlockManifest>> {
        let Some(blockid) = self.get_canonical_blockid_async(height).await? else {
            return Ok(None);
        };

        self.get_block_manifest_async(&blockid).await
    }

    pub fn get_canonical_blockid(&self, height: u64) -> DbResult<Option<L1BlockId>> {
        self.blockheight_cache
            .get_or_fetch_blocking(&height, || self.ops.get_canonical_blockid_blocking(height))
    }

    pub async fn get_canonical_blockid_async(&self, height: u64) -> DbResult<Option<L1BlockId>> {
        self.blockheight_cache
            .get_or_fetch(&height, || self.ops.get_canonical_blockid_chan(height))
            .await
    }

    pub fn get_canonical_blockid_range(
        &self,
        start_idx: u64,
        end_idx: u64,
    ) -> DbResult<Vec<L1BlockId>> {
        self.ops
            .get_canonical_blockid_range_blocking(start_idx, end_idx)
    }

    pub async fn get_canonical_blockid_range_async(
        &self,
        start_idx: u64,
        end_idx: u64,
    ) -> DbResult<Vec<L1BlockId>> {
        self.ops
            .get_canonical_blockid_range_async(start_idx, end_idx)
            .await
    }

    pub fn get_block_txs(&self, blockid: &L1BlockId) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch_blocking(blockid, || self.ops.get_block_txs_blocking(*blockid))
    }

    pub async fn get_block_txs_async(&self, blockid: &L1BlockId) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch(blockid, || self.ops.get_block_txs_chan(*blockid))
            .await
    }

    pub fn get_block_txs_at_height(&self, height: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        let Some(blockid) = self.get_canonical_blockid(height)? else {
            return Ok(None);
        };
        self.get_block_txs(&blockid)
    }

    pub async fn get_block_txs_at_height_async(
        &self,
        height: u64,
    ) -> DbResult<Option<Vec<L1TxRef>>> {
        let Some(blockid) = self.get_canonical_blockid_async(height).await? else {
            return Ok(None);
        };
        self.get_block_txs_async(&blockid).await
    }

    pub fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_blocking(tx_ref)
    }

    pub async fn get_tx_async(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_async(tx_ref).await
    }
}
