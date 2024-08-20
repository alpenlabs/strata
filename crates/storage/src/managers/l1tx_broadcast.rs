//! L1 tx Broadcast data manager.

// NOTE: As an existing convention, this should actually be in ops, but we don't need caching layer
// sofar thus this should be fine.

use std::sync::Arc;

use alpen_express_db::{
    traits::*,
    types::{L1TxEntry, L1TxStatus},
};
use alpen_express_primitives::buf::Buf32;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct L1BroadcastContext<D: TxBroadcastDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: TxBroadcastDatabase + Sync + Send + 'static> L1BroadcastContext<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BroadcastDbManager {
        BroadcastDbManager::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BroadcastDbManager, L1BroadcastContext<D: TxBroadcastDatabase>) {
        get_tx_entry(idx: u64) => Option<L1TxEntry>;
        get_tx_status(id: Buf32) => Option<L1TxStatus>;
        get_next_tx_idx() => u64;
        insert_new_tx_entry(id: Buf32, entry: L1TxEntry) => u64;
        update_tx_entry(idx: u64, entry: L1TxEntry) => ();
    }
}

fn get_tx_entry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &L1BroadcastContext<D>,
    idx: u64,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_tx_entry(idx)
}

fn get_tx_status<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &L1BroadcastContext<D>,
    id: Buf32,
) -> DbResult<Option<L1TxStatus>> {
    let bcast_prov = context.db.broadcast_provider();
    Ok(bcast_prov.get_tx_entry_by_id(id)?.map(|entry| entry.status))
}

fn get_next_tx_idx<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &L1BroadcastContext<D>,
) -> DbResult<u64> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_next_tx_idx()
}

fn insert_new_tx_entry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &L1BroadcastContext<D>,
    id: Buf32,
    entry: L1TxEntry,
) -> DbResult<u64> {
    let bcast_store = context.db.broadcast_store();
    bcast_store.insert_new_tx_entry(id, entry)
}

fn update_tx_entry<D: TxBroadcastDatabase + Sync + Send + 'static>(
    context: &L1BroadcastContext<D>,
    idx: u64,
    entry: L1TxEntry,
) -> DbResult<()> {
    let bcast_store = context.db.broadcast_store();
    bcast_store.update_tx_entry(idx, entry)
}
