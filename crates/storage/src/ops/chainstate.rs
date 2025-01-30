//! Chainstate database low-level operations wrapper.

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::{chain_state::Chainstate, state_op::WriteBatch};

use crate::exec::*;

pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> ChainstateDataOps {
        ChainstateDataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (ChainstateDataOps, Context<D: Database>) {
        write_genesis_state(toplevel: Chainstate) => ();
        write_state_update(idx: u64, batch: WriteBatch) => ();
        get_last_state_idx() => u64;
        get_earliest_state_idx() => u64;
        get_writes_at(idx: u64) => Option<WriteBatch>;
        get_toplevel_state(idx: u64) => Option<Chainstate>;
        // TODO the rest, not including yet because might iterate
    }
}

fn write_genesis_state<D: Database>(context: &Context<D>, toplevel: Chainstate) -> DbResult<()> {
    let chs = context.db.chain_state_db();
    chs.write_genesis_state(&toplevel)?;
    Ok(())
}

fn write_state_update<D: Database>(
    context: &Context<D>,
    idx: u64,
    batch: WriteBatch,
) -> DbResult<()> {
    let chs = context.db.chain_state_db();
    chs.write_state_update(idx, &batch)?;
    Ok(())
}

fn get_last_state_idx<D: Database>(context: &Context<D>) -> DbResult<u64> {
    context.db.chain_state_db().get_last_state_idx()
}

fn get_earliest_state_idx<D: Database>(context: &Context<D>) -> DbResult<u64> {
    context.db.chain_state_db().get_earliest_state_idx()
}

fn get_writes_at<D: Database>(context: &Context<D>, idx: u64) -> DbResult<Option<WriteBatch>> {
    context.db.chain_state_db().get_writes_at(idx)
}

fn get_toplevel_state<D: Database>(context: &Context<D>, idx: u64) -> DbResult<Option<Chainstate>> {
    context.db.chain_state_db().get_toplevel_state(idx)
}
