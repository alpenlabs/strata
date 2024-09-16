use std::sync::Arc;

use alpen_express_db::{
    entities::bridge_tx_state::BridgeTxState, traits::BridgeTxDatabase, DbResult,
};
use bitcoin::Txid;

use crate::exec::*;

/// Database context for a database operation interface.
pub struct Context<D: BridgeTxDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: BridgeTxDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BridgeTxStateOps {
        BridgeTxStateOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BridgeTxStateOps, Context<D: BridgeTxDatabase>) {
        get_tx_state(txid: Txid) => Option<BridgeTxState>;
        upsert_tx_state(txid: Txid, tx_state: BridgeTxState) => ();
    }
}

fn get_tx_state<D: BridgeTxDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Txid,
) -> DbResult<Option<BridgeTxState>> {
    context.db.get_tx_state(txid.into())
}

fn upsert_tx_state<D: BridgeTxDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Txid,
    tx_state: BridgeTxState,
) -> DbResult<()> {
    context.db.put_tx_state(txid.into(), tx_state)
}
