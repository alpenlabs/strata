//! Low level database ops for chainstate database.

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::{chain_state::Chainstate, state_op::WriteBatch};

use crate::exec::*;

inst_ops_simple! {
    (<D: ChainstateDatabase> => ChainstateOps) {
        write_genesis_state(toplevel: Chainstate) => ();
        write_state_update(idx: u64, batch: WriteBatch) => ();
        purge_historical_state_before(before_idx: u64) => ();
        rollback_writes_to(new_tip_idx: u64) => ();
        get_last_state_idx() => u64;
        get_earliest_state_idx() => u64;
        get_writes_at(idx: u64) => Option<WriteBatch>;
        get_toplevel_state(idx: u64) => Option<Chainstate>;
    }
}
