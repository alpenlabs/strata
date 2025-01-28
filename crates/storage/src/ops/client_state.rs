//! Client data database operations interface..

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::{
    client_state::ClientState,
    operation::{ClientStateWrite, ClientUpdateOutput, SyncAction},
};

use crate::exec::*;

inst_ops_simple! {
    (<D: ClientStateDatabase> => ClientStateOps) {
        write_client_update_output(idx: u64, output: ClientUpdateOutput) => ();
        write_client_state_checkpoint(idx: u64, state: ClientState) => ();
        get_last_write_idx() => u64;
        get_client_state_writes(idx: u64) => Option<Vec<ClientStateWrite>>;
        get_client_update_actions(idx: u64) => Option<Vec<SyncAction>>;
        get_last_checkpoint_idx() => u64;
        get_prev_checkpoint_at(idx: u64) => u64;
        get_state_checkpoint(idx: u64) => Option<ClientState>;
    }
}
