use std::sync::Arc;

use alpen_express_primitives::buf::Buf32;
use tokio::sync::oneshot;
use tracing::*;

use alpen_express_db::{
    errors::DbError,
    traits::{BcastProvider, BcastStore, TxBroadcastDatabase},
    types::L1TxEntry,
    DbResult,
};

pub struct BroadcastManager {
    get_tx_shim: Shim<u64, Option<L1TxEntry>>,
    add_tx_shim: Shim<(Buf32, L1TxEntry), u64>,
    put_tx_shim: Shim<(u64, L1TxEntry), ()>,
}

impl BroadcastManager {
    pub fn new<D: TxBroadcastDatabase + Send + Sync + 'static>(
        db: Arc<D>,
        pool: Arc<threadpool::ThreadPool>,
    ) -> Self {
        Self {
            get_tx_shim: make_get_tx_shim(db.clone(), pool.clone()),
            put_tx_shim: make_put_tx_shim(db.clone(), pool.clone()),
            add_tx_shim: make_add_tx_shim(db.clone(), pool.clone()),
        }
    }

    pub fn get_tx_by_idx(&self, idx: u64) -> DbResult<Option<L1TxEntry>> {
        (self.get_tx_shim.handle)(idx).wait_blocking()
    }

    pub async fn get_tx_by_idx_async(&self, idx: u64) -> DbResult<Option<L1TxEntry>> {
        (self.get_tx_shim.handle)(idx).wait().await
    }

    pub fn put_tx(&self, idx: u64, entry: L1TxEntry) -> DbResult<()> {
        (self.put_tx_shim.handle)((idx, entry)).wait_blocking()
    }

    pub async fn put_tx_async(&self, idx: u64, entry: L1TxEntry) -> DbResult<()> {
        (self.put_tx_shim.handle)((idx, entry)).wait().await
    }

    pub fn add_tx(&self, id: Buf32, entry: L1TxEntry) -> DbResult<u64> {
        (self.add_tx_shim.handle)((id, entry)).wait_blocking()
    }

    pub async fn add_tx_async(&self, id: Buf32, entry: L1TxEntry) -> DbResult<u64> {
        (self.add_tx_shim.handle)((id, entry)).wait().await
    }
}

struct Shim<T, R> {
    handle: Box<dyn Fn(T) -> BroadcastHandle<R> + Sync + Send + 'static>,
}

struct BroadcastHandle<R> {
    resp_rx: oneshot::Receiver<DbResult<R>>,
}

impl<R> BroadcastHandle<R> {
    pub fn wait_blocking(self) -> DbResult<R> {
        match self.resp_rx.blocking_recv() {
            Ok(v) => v,
            Err(e) => Err(DbError::Other(format!("{e}"))),
        }
    }

    pub async fn wait(self) -> DbResult<R> {
        match self.resp_rx.await {
            Ok(v) => v,
            Err(e) => Err(DbError::Other(format!("{e}"))),
        }
    }
}

fn make_get_tx_shim<D: TxBroadcastDatabase + Sync + Send + 'static>(
    db: Arc<D>,
    pool: Arc<threadpool::ThreadPool>,
) -> Shim<u64, Option<L1TxEntry>> {
    let fun = move |idx| {
        let db = db.clone();
        let (resp_tx, resp_rx) = oneshot::channel();

        pool.execute(move || {
            let bprov = db.broadcast_provider();
            let res = bprov.get_txentry_by_idx(idx);
            if resp_tx.send(res).is_err() {
                warn!("failed to get txidx");
            }
        });

        BroadcastHandle { resp_rx }
    };

    Shim {
        handle: Box::new(fun),
    }
}

fn make_add_tx_shim<D: TxBroadcastDatabase + Sync + Send + 'static>(
    db: Arc<D>,
    pool: Arc<threadpool::ThreadPool>,
) -> Shim<(Buf32, L1TxEntry), u64> {
    let fun = move |(id, txentry)| {
        let db = db.clone();
        let (resp_tx, resp_rx) = oneshot::channel();

        pool.execute(move || {
            let bstore = db.broadcast_store();
            let res = bstore.add_tx(id, txentry);
            if resp_tx.send(res).is_err() {
                warn!("failed to add tx");
            }
        });

        BroadcastHandle { resp_rx }
    };

    Shim {
        handle: Box::new(fun),
    }
}

fn make_put_tx_shim<D: TxBroadcastDatabase + Sync + Send + 'static>(
    db: Arc<D>,
    pool: Arc<threadpool::ThreadPool>,
) -> Shim<(u64, L1TxEntry), ()> {
    let fun = move |(idx, txentry)| {
        let db = db.clone();
        let (resp_tx, resp_rx) = oneshot::channel();

        pool.execute(move || {
            let bstore = db.broadcast_store();
            let res = bstore.update_tx_by_idx(idx, txentry);
            if resp_tx.send(res).is_err() {
                warn!("failed to update tx");
            }
        });

        BroadcastHandle { resp_rx }
    };

    Shim {
        handle: Box::new(fun),
    }
}
