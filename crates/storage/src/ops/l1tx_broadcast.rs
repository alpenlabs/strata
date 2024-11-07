use std::sync::Arc;

use strata_db::{
    traits::*,
    types::{L1TxEntry, L1TxStatus},
    DbResult,
};
use strata_primitives::buf::Buf32;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: BroadcastDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: BroadcastDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BroadcastDbOps {
        BroadcastDbOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BroadcastDbOps, Context<D: BroadcastDatabase>) {
        get_tx_entry(idx: u64) => Option<L1TxEntry>;
        get_tx_entry_by_id(id: Buf32) => Option<L1TxEntry>;
        get_tx_status(id: Buf32) => Option<L1TxStatus>;
        get_txid(idx: u64) => Option<Buf32>;
        get_next_tx_idx() => u64;
        put_tx_entry(id: Buf32, entry: L1TxEntry) => Option<u64>;
        put_tx_entry_by_idx(idx: u64, entry: L1TxEntry) => ();
    }
}

fn get_tx_entry<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.get_tx_entry(idx)
}

fn get_tx_entry_by_id<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    id: Buf32,
) -> DbResult<Option<L1TxEntry>> {
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.get_tx_entry_by_id(id)
}

fn get_txid<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
) -> DbResult<Option<Buf32>> {
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.get_txid(idx)
}

fn get_tx_status<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    id: Buf32,
) -> DbResult<Option<L1TxStatus>> {
    let bcast_db = context.db.l1_broadcast_db();
    Ok(bcast_db.get_tx_entry_by_id(id)?.map(|entry| entry.status))
}

fn get_next_tx_idx<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
) -> DbResult<u64> {
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.get_next_tx_idx()
}

fn put_tx_entry<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Buf32,
    entry: L1TxEntry,
) -> DbResult<Option<u64>> {
    trace!(%txid, "insert_new_tx_entry");
    assert!(entry.try_to_tx().is_ok(), "invalid tx entry {entry:?}");
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.put_tx_entry(txid, entry)
}

fn put_tx_entry_by_idx<D: BroadcastDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    idx: u64,
    entry: L1TxEntry,
) -> DbResult<()> {
    let bcast_db = context.db.l1_broadcast_db();
    bcast_db.put_tx_entry_by_idx(idx, entry)
}
