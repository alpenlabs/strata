use std::sync::Arc;

use strata_db::{traits::L1Database, DbError, DbResult};
use strata_primitives::l1::{L1BlockId, L1BlockManifest, L1Tx, L1TxRef};
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
    chaintip_cache: CacheTable<(), Option<(u64, L1BlockId)>>,
}

impl L1BlockManager {
    pub fn new<D: L1Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l1::Context::new(db).into_ops(pool);
        let manifest_cache = cache::CacheTable::new(64.try_into().unwrap());
        let txs_cache = cache::CacheTable::new(64.try_into().unwrap());
        let blockheight_cache = cache::CacheTable::new(64.try_into().unwrap());
        let chaintip_cache = cache::CacheTable::new(1.try_into().unwrap());
        Self {
            ops,
            manifest_cache,
            txs_cache,
            blockheight_cache,
            chaintip_cache,
        }
    }

    pub fn put_block_data(&self, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        let blockid = mf.blkid();
        self.manifest_cache.purge(blockid);
        self.txs_cache.purge(blockid);
        self.ops.put_block_data_blocking(mf, txs)
    }

    pub async fn put_block_data_async(&self, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        let blockid = mf.blkid();
        self.manifest_cache.purge(blockid);
        self.txs_cache.purge(blockid);
        self.ops.put_block_data_async(mf, txs).await
    }

    pub fn extend_canonical_chain(&self, blockid: &L1BlockId) -> DbResult<()> {
        let new_block = self
            .get_block_manifest(blockid)?
            .ok_or(DbError::MissingL1BlockBody(*blockid))?;
        let height = new_block.height();

        if let Some((tip_height, tip_blockid)) = self.get_chain_tip()? {
            if height != tip_height + 1 {
                return Err(DbError::OooInsert("l1block", height));
            }

            if new_block.get_prev_blockid() != tip_blockid {
                return Err(DbError::Other(format!(
                    "l1block does not extend chain {blockid}"
                )));
            }

            for i in height + 1..=tip_height {
                self.blockheight_cache.purge(&i);
            }
        };

        self.chaintip_cache.purge(&());
        self.ops
            .set_canonical_chain_entry_blocking(height, *blockid)
    }

    pub async fn extend_canonical_chain_async(&self, blockid: &L1BlockId) -> DbResult<()> {
        let new_block = self
            .get_block_manifest_async(blockid)
            .await?
            .ok_or(DbError::MissingL1BlockBody(*blockid))?;
        let height = new_block.height();

        if let Some((tip_height, tip_blockid)) = self.get_chain_tip_async().await? {
            if height != tip_height + 1 {
                return Err(DbError::OooInsert("l1block", height));
            }

            if new_block.get_prev_blockid() != tip_blockid {
                return Err(DbError::Other(format!(
                    "l1block does not extend chain {blockid}"
                )));
            }

            for i in height + 1..=tip_height {
                self.blockheight_cache.purge(&i);
            }
        };

        self.chaintip_cache.purge(&());
        self.ops
            .set_canonical_chain_entry_async(height, *blockid)
            .await
    }

    pub fn revert_canonical_chain(&self, height: u64) -> DbResult<()> {
        let Some((tip_height, _)) = self.ops.get_chain_tip_blocking()? else {
            // no chain to revert
            // but clear cache anyway
            self.blockheight_cache.clear();
            self.chaintip_cache.clear();
            return Ok(());
        };

        for i in height + 1..=tip_height {
            self.blockheight_cache.purge(&i);
        }
        self.chaintip_cache.purge(&());
        self.ops
            .remove_canonical_chain_range_blocking(height + 1, tip_height)
    }

    pub async fn revert_canonical_chain_async(&self, height: u64) -> DbResult<()> {
        let Some((tip_height, _)) = self.ops.get_chain_tip_async().await? else {
            // no chain to revert
            // but clear cache anyway
            self.blockheight_cache.clear();
            self.chaintip_cache.clear();
            return Ok(());
        };

        for i in height + 1..=tip_height {
            self.blockheight_cache.purge(&i);
        }
        self.chaintip_cache.purge(&());
        self.ops
            .remove_canonical_chain_range_async(height + 1, tip_height)
            .await
    }

    pub fn get_chain_tip(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.chaintip_cache
            .get_or_fetch_blocking(&(), || self.ops.get_chain_tip_blocking())
    }

    pub async fn get_chain_tip_async(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.chaintip_cache
            .get_or_fetch(&(), || self.ops.get_chain_tip_chan())
            .await
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
