use std::sync::Arc;

use strata_db::{traits::L1Database, DbError, DbResult};
use strata_primitives::l1::{L1Block, L1BlockId, L1BlockManifest, L1Tx, L1TxRef};
use threadpool::ThreadPool;
use tracing::error;

use crate::{cache::CacheTable, ops};

/// Caching manager of L1 block data
pub struct L1BlockManager {
    ops: ops::l1::L1DataOps,
    manifest_cache: CacheTable<L1BlockId, Option<L1BlockManifest>>,
    block_cache: CacheTable<L1BlockId, Option<L1Block>>,
    txs_cache: CacheTable<L1BlockId, Option<Vec<L1TxRef>>>,
    blockheight_cache: CacheTable<u64, Option<L1BlockId>>,
}

impl L1BlockManager {
    /// Create new instance of [`L1BlockManager`]
    pub fn new<D: L1Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l1::Context::new(db).into_ops(pool);
        let manifest_cache = CacheTable::new(64.try_into().unwrap());
        let block_cache = CacheTable::new(64.try_into().unwrap());
        let txs_cache = CacheTable::new(64.try_into().unwrap());
        let blockheight_cache = CacheTable::new(64.try_into().unwrap());
        Self {
            ops,
            manifest_cache,
            block_cache,
            txs_cache,
            blockheight_cache,
        }
    }

    /// Save an [`L1BlockManifest`] to database. Does not add block to tracked canonical chain.
    pub fn put_block_data(&self, mf: L1BlockManifest) -> DbResult<()> {
        let blockid = mf.blkid();
        self.manifest_cache.purge(blockid);
        self.txs_cache.purge(blockid);
        self.ops.put_block_data_blocking(mf)
    }

    /// Save an [`L1BlockManifest`] to database. Does not add block to tracked canonical chain.
    pub async fn put_block_data_async(&self, mf: L1BlockManifest) -> DbResult<()> {
        let blockid = mf.blkid();
        self.manifest_cache.purge(blockid);
        self.txs_cache.purge(blockid);
        self.ops.put_block_data_async(mf).await
    }

    pub fn put_block(&self, block: L1Block) -> DbResult<()> {
        let blockid = &block.block_id();
        self.block_cache.purge(blockid);
        self.ops.put_block_blocking(block)
    }

    pub async fn put_block_async(&self, block: L1Block) -> DbResult<()> {
        let blockid = &block.block_id();
        self.block_cache.purge(blockid);
        self.ops.put_block_async(block).await
    }

    /// Append `blockid` to tracked canonical chain.
    /// [`L1BlockManifest`] for this `blockid` must be present in db.
    pub fn extend_canonical_chain(&self, blockid: &L1BlockId) -> DbResult<()> {
        let new_block = self
            .get_block_manifest(blockid)?
            .ok_or(DbError::MissingL1BlockManifest(*blockid))?;
        let height = new_block.height();

        if let Some((tip_height, tip_blockid)) = self.get_canonical_chain_tip()? {
            if height != tip_height + 1 {
                error!(expected = %(tip_height + 1), got = %height, "attempted to extend canonical chain out of order");
                return Err(DbError::OooInsert("l1block", height));
            }

            if new_block.get_prev_blockid() != tip_blockid {
                return Err(DbError::L1InvalidNextBlock(height, *blockid));
            }
        };

        self.ops
            .set_canonical_chain_entry_blocking(height, *blockid)
    }

    /// Append `blockid` to tracked canonical chain.
    /// [`L1BlockManifest`] for this `blockid` must be present in db.
    pub async fn extend_canonical_chain_async(&self, blockid: &L1BlockId) -> DbResult<()> {
        let new_block = self
            .get_block_manifest_async(blockid)
            .await?
            .ok_or(DbError::MissingL1BlockManifest(*blockid))?;
        let height = new_block.height();

        if let Some((tip_height, tip_blockid)) = self.get_canonical_chain_tip_async().await? {
            if height != tip_height + 1 {
                error!(expected = %(tip_height + 1), got = %height, "attempted to extend canonical chain out of order");
                return Err(DbError::OooInsert("l1block", height));
            }

            if new_block.get_prev_blockid() != tip_blockid {
                return Err(DbError::L1InvalidNextBlock(height, *blockid));
            }
        };

        self.ops
            .set_canonical_chain_entry_async(height, *blockid)
            .await
    }

    /// Reverts tracked canonical chain to `height`.
    /// `height` must be less than tracked canonical chain height.
    pub fn revert_canonical_chain(&self, height: u64) -> DbResult<()> {
        let Some((tip_height, _)) = self.ops.get_canonical_chain_tip_blocking()? else {
            // no chain to revert
            // but clear cache anyway for sanity
            self.blockheight_cache.clear();
            return Err(DbError::L1CanonicalChainEmpty);
        };

        if height > tip_height {
            return Err(DbError::L1InvalidRevertHeight(height, tip_height));
        }

        // clear item from cache for range height +1..=tip_height
        self.blockheight_cache
            .purge_if(|h| height < *h && *h <= tip_height);

        self.ops
            .remove_canonical_chain_entries_blocking(height + 1, tip_height)
    }

    /// Reverts tracked canonical chain to `height`.
    /// `height` must be less than tracked canonical chain height.
    pub async fn revert_canonical_chain_async(&self, height: u64) -> DbResult<()> {
        let Some((tip_height, _)) = self.ops.get_canonical_chain_tip_async().await? else {
            // no chain to revert
            // but clear cache anyway for sanity
            self.blockheight_cache.clear();

            return Err(DbError::L1CanonicalChainEmpty);
        };

        if height > tip_height {
            return Err(DbError::L1InvalidRevertHeight(height, tip_height));
        }

        // clear item from cache for range height +1..=tip_height
        self.blockheight_cache
            .purge_if(|h| height < *h && *h <= tip_height);

        self.ops
            .remove_canonical_chain_entries_async(height + 1, tip_height)
            .await
    }

    // Get tracked canonical chain tip height and blockid.
    pub fn get_canonical_chain_tip(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.ops.get_canonical_chain_tip_blocking()
    }

    // Get tracked canonical chain tip height and blockid.
    pub async fn get_canonical_chain_tip_async(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.ops.get_canonical_chain_tip_async().await
    }

    // Get tracked canonical chain tip height.
    pub fn get_chain_tip_height(&self) -> DbResult<Option<u64>> {
        Ok(self.get_canonical_chain_tip()?.map(|(height, _)| height))
    }

    // Get tracked canonical chain tip height.
    pub async fn get_chain_tip_height_async(&self) -> DbResult<Option<u64>> {
        Ok(self
            .get_canonical_chain_tip_async()
            .await?
            .map(|(height, _)| height))
    }

    // Get [`L1BlockManifest`] for given `blockid`.
    pub fn get_block_manifest(&self, blockid: &L1BlockId) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch_blocking(blockid, || self.ops.get_block_manifest_blocking(*blockid))
    }

    // Get [`L1BlockManifest`] for given `blockid`.
    pub async fn get_block_manifest_async(
        &self,
        blockid: &L1BlockId,
    ) -> DbResult<Option<L1BlockManifest>> {
        self.manifest_cache
            .get_or_fetch(blockid, || self.ops.get_block_manifest_chan(*blockid))
            .await
    }

    // Get [`L1BlockManifest`] at `height` in tracked canonical chain.
    pub fn get_block_manifest_at_height(&self, height: u64) -> DbResult<Option<L1BlockManifest>> {
        let Some(blockid) = self.get_canonical_blockid_at_height(height)? else {
            return Ok(None);
        };

        self.get_block_manifest(&blockid)
    }

    // Get [`L1BlockManifest`] at `height` in tracked canonical chain.
    pub async fn get_block_manifest_at_height_async(
        &self,
        height: u64,
    ) -> DbResult<Option<L1BlockManifest>> {
        let Some(blockid) = self.get_canonical_blockid_at_height_async(height).await? else {
            return Ok(None);
        };

        self.get_block_manifest_async(&blockid).await
    }

    // Get [`L1Block`] for given `blockid`.
    pub fn get_block(&self, blockid: &L1BlockId) -> DbResult<Option<L1Block>> {
        self.block_cache
            .get_or_fetch_blocking(blockid, || self.ops.get_block_blocking(*blockid))
    }

    // Get [`L1Block`] for given `blockid`.
    pub async fn get_block_async(&self, blockid: &L1BlockId) -> DbResult<Option<L1Block>> {
        self.block_cache
            .get_or_fetch(blockid, || self.ops.get_block_chan(*blockid))
            .await
    }

    // Get [`L1Block`] at `height` in tracked canonical chain.
    pub fn get_block_at_height(&self, height: u64) -> DbResult<Option<L1Block>> {
        let Some(blockid) = self.get_canonical_blockid_at_height(height)? else {
            return Ok(None);
        };

        self.get_block(&blockid)
    }

    // Get [`L1Block`] at `height` in tracked canonical chain.
    pub async fn get_block_at_height_async(&self, height: u64) -> DbResult<Option<L1Block>> {
        let Some(blockid) = self.get_canonical_blockid_at_height_async(height).await? else {
            return Ok(None);
        };

        self.get_block_async(&blockid).await
    }

    // Get [`L1BlockId`] at `height` in tracked canonical chain.
    pub fn get_canonical_blockid_at_height(&self, height: u64) -> DbResult<Option<L1BlockId>> {
        self.blockheight_cache.get_or_fetch_blocking(&height, || {
            self.ops.get_canonical_blockid_at_height_blocking(height)
        })
    }

    // Get [`L1BlockId`] at `height` in tracked canonical chain.
    pub async fn get_canonical_blockid_at_height_async(
        &self,
        height: u64,
    ) -> DbResult<Option<L1BlockId>> {
        self.blockheight_cache
            .get_or_fetch(&height, || {
                self.ops.get_canonical_blockid_at_height_chan(height)
            })
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

    // Get indexed transasction inside `blockid`.
    pub fn get_block_txs(&self, blockid: &L1BlockId) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch_blocking(blockid, || self.ops.get_block_txs_blocking(*blockid))
    }

    // Get indexed transasction inside `blockid`.
    pub async fn get_block_txs_async(&self, blockid: &L1BlockId) -> DbResult<Option<Vec<L1TxRef>>> {
        self.txs_cache
            .get_or_fetch(blockid, || self.ops.get_block_txs_chan(*blockid))
            .await
    }

    // Get indexed transasction inside `blockid`.
    pub fn get_block_txs_at_height(&self, height: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        let Some(blockid) = self.get_canonical_blockid_at_height(height)? else {
            return Ok(None);
        };
        self.get_block_txs(&blockid)
    }

    // Get indexed transasction inside block at `height` in tracked canonical chain.
    pub async fn get_block_txs_at_height_async(
        &self,
        height: u64,
    ) -> DbResult<Option<Vec<L1TxRef>>> {
        let Some(blockid) = self.get_canonical_blockid_at_height_async(height).await? else {
            return Ok(None);
        };
        self.get_block_txs_async(&blockid).await
    }

    // Get indexed transaction identified by `tx_ref`.
    pub fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_blocking(tx_ref)
    }

    // Get indexed transaction identified by `tx_ref`.
    pub async fn get_tx_async(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        // TODO: Might need to use a cache here, but let's keep it for when we use it
        self.ops.get_tx_async(tx_ref).await
    }
}
