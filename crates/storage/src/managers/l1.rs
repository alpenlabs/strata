use std::sync::Arc;

use async_trait::async_trait;
use strata_db::{
    traits::{BlockStatus, Database, L1Database},
    DbResult,
};
use strata_mmr::CompactMmr;
use strata_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1TxRef},
};
use strata_state::{block::L2BlockBundle, header::L2Header, id::L2BlockId, l1::L1Tx};
use threadpool::ThreadPool;
use tokio::sync::oneshot::{self, error::RecvError};
use tracing::warn;

use crate::cache::{self, CacheTable};

/// Caching manager of L1 blocks in the block database.
pub struct L1BlockManager<DB>
where
    DB: L1Database + Sync + Send + 'static,
{
    pool: ThreadPool,
    db: Arc<DB>,
    block_cache: CacheTable<L2BlockId, Option<L2BlockBundle>>,
}

impl<DB> L1BlockManager<DB>
where
    DB: L1Database + Sync + Send + 'static,
{
    pub fn new(pool: ThreadPool, db: Arc<DB>) -> Self {
        Self {
            pool,
            db,
            block_cache: CacheTable::new(64.try_into().unwrap()),
        }
    }

    pub async fn put_block_data(
        &self,
        idx: u64,
        mf: L1BlockManifest,
        txs: Vec<L1Tx>,
    ) -> DbResult<()> {
        let db = self.db.clone();
        self.pool
            .spawn(move || db.put_block_data(idx, mf, txs))
            .await
    }

    pub async fn put_mmr_checkpoint(&self, idx: u64, mmr: CompactMmr) -> DbResult<()> {
        let db = self.db.clone();
        self.pool
            .spawn(move || db.put_mmr_checkpoint(idx, mmr))
            .await
    }

    pub async fn revert_to_height(&self, idx: u64) -> DbResult<()> {
        let db = self.db.clone();
        self.pool.spawn(move || db.revert_to_height(idx)).await
    }

    pub async fn get_chain_tip(&self) -> DbResult<Option<u64>> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_chain_tip()).await
    }

    pub async fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_block_manifest(idx)).await
    }

    pub async fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>> {
        let db = self.db.clone();
        self.pool
            .spawn(move || db.get_blockid_range(start_idx, end_idx))
            .await
    }

    pub async fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_block_txs(idx)).await
    }

    pub async fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_tx(tx_ref)).await
    }

    pub async fn get_last_mmr_to(&self, idx: u64) -> DbResult<Option<CompactMmr>> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_last_mmr_to(idx)).await
    }

    pub async fn get_txs_from(&self, start_idx: u64) -> DbResult<(Vec<L1Tx>, u64)> {
        let db = self.db.clone();
        self.pool.spawn(move || db.get_txs_from(start_idx)).await
    }
}

#[async_trait]
trait ThreadPoolSpawn {
    async fn spawn<T, F>(&self, func: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
}

#[async_trait]
impl ThreadPoolSpawn for ThreadPool {
    async fn spawn<T, F>(&self, func: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.execute(move || {
            if tx.send(func()).is_err() {
                warn!("failed to send response")
            }
        });
        rx.await.expect("Sender was dropped without sending")
    }
}
