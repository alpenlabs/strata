//! Describes precondition checks that could prevent a sync event from being
//! executed.

use serde::{Deserialize, Serialize};
use strata_state::{l1::L1BlockId, sync_event::SyncEvent};

use crate::errors::Error;

/// Precondtion predicate we can evaluate over the current state.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum SyncPrecond {
    /// Must have L1 blocks up to some height in the database.
    L1BlockHeight(u64),

    /// Must have a particular L1 blockid in the database.
    L1BlockPresent(u64, L1BlockId),
}

/// Contains infra for checking preconditions.
pub struct PrecondChecker {
    // TODO
}

impl PrecondChecker {
    /// Checks the precondition and returning if it passes or not, or returns an error.
    pub fn check_precond(&self, precond: &SyncPrecond) -> Result<bool, Error> {
        Ok(true)
    }
}

/// Computes a list of preconditions for some sync event data.
pub fn compute_sync_event_preconditions(ev: &SyncEvent) -> Vec<SyncPrecond> {
    let p = match ev {
        SyncEvent::L1Block(h, id) => SyncPrecond::L1BlockPresent(*h, *id),
        SyncEvent::L1Revert(h) => SyncPrecond::L1BlockHeight(*h),
        SyncEvent::L1DABatch(h, _) => SyncPrecond::L1BlockHeight(*h),
        SyncEvent::L1BlockGenesis(h, _) => SyncPrecond::L1BlockHeight(*h),

        // By default we don't produce any preconditions.
        _ => return Vec::new(),
    };

    vec![p]
}
