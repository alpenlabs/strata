use std::sync::Arc;

use strata_db::{traits::*, types::L1TxEntry, DbResult};
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

    pub fn db(&self) -> &impl L1BroadcastDatabase {
        self.db.l1_broadcast_db().as_ref()
    }
}

inst_ops_auto! {
    (BroadcastDbOps, Context<D: BroadcastDatabase>) {
        get_tx_entry(idx: u64) => Option<L1TxEntry>;
        get_tx_entry_by_id(id: Buf32) => Option<L1TxEntry>;
        get_txid(idx: u64) => Option<Buf32>;
        get_next_tx_idx() => u64;
        put_tx_entry(id: Buf32, entry: L1TxEntry) => Option<u64>;
        put_tx_entry_by_idx(idx: u64, entry: L1TxEntry) => ();
    }
}
