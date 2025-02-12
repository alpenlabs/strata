//! Client data database operations interface..

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::operation::ClientUpdateOutput;

use crate::exec::*;

inst_ops_simple! {
    (<D: ClientStateDatabase> => ClientStateOps) {
        put_client_update(idx: u64, output: ClientUpdateOutput) => ();
        get_client_update(idx: u64) => Option<ClientUpdateOutput>;
        get_last_state_idx() => u64;
    }
}
