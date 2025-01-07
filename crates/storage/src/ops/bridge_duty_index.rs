use std::sync::Arc;

use strata_db::{traits::BridgeDutyIndexDatabase, DbResult};

use crate::exec::*;

inst_ops_simple! {
    (<D: BridgeDutyIndexDatabase> => BridgeDutyIndexOps) {
        get_index() => Option<u64>;
        set_index(index: u64) => ();
    }
}
