//! Sync event operations.

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::sync_event::SyncEvent;

use crate::exec::*;

inst_ops_simple! {
    (<D: SyncEventDatabase> => SyncEventOps) {
        write_sync_event(ev: SyncEvent) => u64;
        clear_sync_event_range(start_idx: u64, end_idx: u64) => ();
        get_last_idx() => Option<u64>;
        get_sync_event(idx: u64) => Option<SyncEvent>;
        get_event_timestamp(idx: u64) => Option<u64>;
    }
}
