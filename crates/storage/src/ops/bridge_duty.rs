use std::sync::Arc;

use bitcoin::Txid;
use strata_db::{traits::BridgeDutyDatabase, DbResult};
use strata_state::bridge_duties::BridgeDutyStatus;

use crate::exec::*;

/// Database context for a database operation interface.
pub struct Context<D: BridgeDutyDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: BridgeDutyDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BridgeDutyOps {
        BridgeDutyOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BridgeDutyOps, Context<D: BridgeDutyDatabase>) {
        get_status(txid: Txid) => Option<BridgeDutyStatus>;
        put_duty_status(txid: Txid, status: BridgeDutyStatus) => ();
        delete_duty(txid: Txid) => Option<BridgeDutyStatus>;
    }
}

fn get_status<D: BridgeDutyDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Txid,
) -> DbResult<Option<BridgeDutyStatus>> {
    context.db.get_status(txid.into())
}

fn put_duty_status<D: BridgeDutyDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Txid,
    status: BridgeDutyStatus,
) -> DbResult<()> {
    context.db.put_duty_status(txid.into(), status)
}

fn delete_duty<D: BridgeDutyDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    txid: Txid,
) -> DbResult<Option<BridgeDutyStatus>> {
    context.db.delete_duty(txid.into())
}
