//! Defines traits related to managing the checkpoint for bridge duty executions and
//! some common implementers.

use async_trait::async_trait;
use strata_primitives::buf::Buf32;

use super::errors::TrackerError;

/// Defines functionalities to add, update and query duty statuses.
// TODO: the actual database related traits should go into the `strata-db` and `strata-storage`
// crates.
#[async_trait]
pub trait ManageTaskStatus: Clone + Send + Sync + Sized {
    /// Get the checkpoint block height.
    async fn get_status(&self, task_id: Buf32) -> Result<u64, TrackerError>;

    /// Update the checkpoint block height with new observed height.
    // TODO: the status should be an enum: `Received`, `Pending`, `Completed`.
    async fn update_status(&self, task_id: u64, status: String) -> Result<(), TrackerError>;
}
