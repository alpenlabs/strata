use std::sync::Arc;

use alpen_express_db::{traits::Database, DbResult};
use alpen_express_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1Tx, L1TxRef},
};
use threadpool::ThreadPool;

use crate::{cache, ops};

pub struct L1DataManager {
    ops: ops::l1::L1DataOps,
    mf_cache: cache::CacheTable<u64, Option<L1BlockManifest>>,
}

impl L1DataManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::l1::Context::new(db).into_ops(pool);
        let mf_cache = cache::CacheTable::new(144.try_into().unwrap());
        Self { ops, mf_cache }
    }

    /// Stores a block's manifest and txs.
    pub async fn put_block_data_async(
        &self,
        block_idx: u64,
        mf: L1BlockManifest,
        txs: Vec<L1Tx>,
    ) -> DbResult<()> {
        self.mf_cache
            .insert_async(block_idx, Some(mf.clone()))
            .await;
        self.ops.put_block_data_async(block_idx, mf, txs).await
    }

    /// Stores a block's manifest and txs.
    pub async fn put_block_data_blocking(
        &self,
        block_idx: u64,
        mf: L1BlockManifest,
        txs: Vec<L1Tx>,
    ) -> DbResult<()> {
        self.mf_cache.insert_blocking(block_idx, Some(mf.clone()));
        self.ops.put_block_data_blocking(block_idx, mf, txs)
    }

    /// Gets a block manifest, possibly cached.
    pub async fn get_manifest_async(&self, block_idx: u64) -> DbResult<Option<L1BlockManifest>> {
        self.mf_cache
            .get_or_fetch_async(&block_idx, || self.ops.get_block_manifest_chan(block_idx))
            .await
    }

    /// Gets a block manifest, possibly cached.
    pub fn get_manifest_blocking(&self, block_idx: u64) -> DbResult<Option<L1BlockManifest>> {
        self.mf_cache.get_or_fetch_blocking(&block_idx, || {
            self.ops.get_block_manifest_blocking(block_idx)
        })
    }

    /// Discards all blocks above the given height, purging the cache.
    pub async fn revert_to_height_async(&self, block_idx: u64) -> DbResult<()> {
        let cur_height = self.ops.get_chain_tip_async().await?;
        self.ops.revert_to_height_async(block_idx).await?;

        // TODO try to not purge older entries that didn't get evicted
        if let Some(prev_height) = cur_height {
            if block_idx < prev_height {
                self.mf_cache.purge_all_async().await;
            }
        }

        Ok(())
    }

    /// Discards all blocks above the given height, purging the cache.
    pub fn revert_to_height_blocking(&self, block_idx: u64) -> DbResult<()> {
        let cur_height = self.ops.get_chain_tip_blocking()?;
        self.ops.revert_to_height_blocking(block_idx)?;

        // TODO try to not purge older entries that didn't get evicted
        if let Some(prev_height) = cur_height {
            if block_idx < prev_height {
                self.mf_cache.purge_all_blocking();
            }
        }

        Ok(())
    }

    /// Gets the current stored L1 chain tip.
    pub async fn get_chain_tip_async(&self) -> DbResult<Option<u64>> {
        self.ops.get_chain_tip_async().await
    }

    /// Gets the current stored L1 chain tip.
    pub fn get_chain_tip_blocking(&self) -> DbResult<Option<u64>> {
        self.ops.get_chain_tip_blocking()
    }

    /// Gets the blkids for a range of blocks, if present.
    pub async fn get_blkid_range_async(&self, start: u64, end: u64) -> DbResult<Vec<Buf32>> {
        self.ops.get_blkid_range_async(start, end).await
    }

    /// Gets the blkids for a range of blocks, if present.
    pub fn get_blkid_range_blocking(&self, start: u64, end: u64) -> DbResult<Vec<Buf32>> {
        self.ops.get_blkid_range_blocking(start, end)
    }

    // TODO possibly convert these to cache the results, I'm not sure how much we'd benefit from
    // doing this though
    /// Gets refs to the remembered transactions in the block.
    pub async fn get_block_txs_async(&self, blk_idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        self.ops.get_block_txs_async(blk_idx).await
    }

    /// Gets refs to the remembered transactions in the block.
    pub async fn get_block_txs_blocking(&self, blk_idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        self.ops.get_block_txs_blocking(blk_idx)
    }

    /// Gets a tx that we've recorded from a block.
    pub async fn get_tx_async(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        self.ops.get_tx_async(tx_ref).await
    }

    /// Gets a tx that we've recorded from a block.
    pub fn get_tx_blocking(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        self.ops.get_tx_blocking(tx_ref)
    }
}
