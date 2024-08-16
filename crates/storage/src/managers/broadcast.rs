//! L1 tx Broadcast data manager.

// NOTE: As an existing convention, this should actuall be in ops, but we don't need caching layer
// sofar thus this should be fine.

use std::sync::Arc;

use alpen_express_db::{traits::*, types::L1TxEntry};
use alpen_express_primitives::buf::Buf32;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct BroadcastContext<D: TxBroadcastDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: TxBroadcastDatabase + Sync + Send + 'static> BroadcastContext<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BroadcastDbManager {
        BroadcastDbManager::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BroadcastDbManager, BroadcastContext<D: TxBroadcastDatabase>) {
        get_txentry(idx: u64) => Option<L1TxEntry>;
        get_last_txidx() => Option<u64>;
        add_txentry(id: Buf32, entry: L1TxEntry) => u64;
        put_txentry(idx: u64, entry: L1TxEntry) => ();
    }
}

fn get_txentry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &BroadcastContext<D>,
    idx: u64,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_txentry_by_idx(idx)
}

fn get_last_txidx<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &BroadcastContext<D>,
) -> DbResult<Option<u64>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_last_txidx()
}

fn add_txentry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &BroadcastContext<D>,
    id: Buf32,
    entry: L1TxEntry,
) -> DbResult<u64> {
    let bcast_store = context.db.broadcast_store();
    bcast_store.add_tx(id, entry)
}

fn put_txentry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &BroadcastContext<D>,
    idx: u64,
    entry: L1TxEntry,
) -> DbResult<()> {
    let bcast_store = context.db.broadcast_store();
    bcast_store.update_tx_by_idx(idx, entry)
}
