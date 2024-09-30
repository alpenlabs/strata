use std::sync::Arc;

use alpen_express_db::{
    traits::*,
    types::{L1TxEntry, L1TxStatus},
    DbResult,
};
use alpen_express_primitives::buf::Buf32;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: L1BroadcastDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: L1BroadcastDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BroadcastDbOps {
        BroadcastDbOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BroadcastDbOps, Context<D: L1BroadcastDatabase>) {
        get_tx_entry(idx: u64) => Option<L1TxEntry>;
        get_tx_entry_by_id(id: Buf32) => Option<L1TxEntry>;
        get_tx_status(id: Buf32) => Option<L1TxStatus>;
        get_txid(idx: u64) => Option<Buf32>;
        get_next_tx_idx() => u64;
        put_tx_entry(id: Buf32, entry: L1TxEntry) => Option<u64>;
        put_tx_entry_by_idx(idx: u64, entry: L1TxEntry) => ();
    }
}

fn get_tx_entry<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_tx_entry(idx)
}

fn get_tx_entry_by_id<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    id: Buf32,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_tx_entry_by_id(id)
}

fn get_txid<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
) -> DbResult<Option<Buf32>> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_txid(idx)
}

fn get_tx_status<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    id: Buf32,
) -> DbResult<Option<L1TxStatus>> {
    let bcast_prov = context.db.broadcast_provider();
    Ok(bcast_prov.get_tx_entry_by_id(id)?.map(|entry| entry.status))
}

fn get_next_tx_idx<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
) -> DbResult<u64> {
    let bcast_prov = context.db.broadcast_provider();
    bcast_prov.get_next_tx_idx()
}

fn put_tx_entry<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Buf32,
    entry: L1TxEntry,
) -> DbResult<Option<u64>> {
    trace!(%txid, "insert_new_tx_entry");
    assert!(entry.try_to_tx().is_ok(), "invalid tx entry {entry:?}");
    let bcast_store = context.db.broadcast_store();
    bcast_store.put_tx_entry(txid, entry)
}

fn put_tx_entry_by_idx<D: L1BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
    entry: L1TxEntry,
) -> DbResult<()> {
    let bcast_store = context.db.broadcast_store();
    bcast_store.put_tx_entry_by_idx(idx, entry)
}
