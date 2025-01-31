//! Low level database ops for chainstate database.

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::{chain_state::Chainstate, state_op::WriteBatch};

use crate::exec::*;

inst_ops_simple! {
    (<D: ChainstateDatabase> => ChainstateOps) {
        write_genesis_state(toplevel: Chainstate) => ();
        put_write_batch(idx: u64, batch: WriteBatch) => ();
        get_write_batch(idx: u64) => Option<WriteBatch>;
        purge_entries_before(before_idx: u64) => ();
        rollback_writes_to(new_tip_idx: u64) => ();
        get_last_write_idx() => u64;
        get_earliest_write_idx() => u64;
    }
}
