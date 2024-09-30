//! Defines traits related to managing the checkpoint for bridge duty executions and
//! some common implementers.

use alpen_express_primitives::l1::OutputRef;
use alpen_express_state::bridge_duties::BridgeDutyStatus;
use async_trait::async_trait;

use super::errors::TrackerError;

/// Defines functionalities to add, update and query duty statuses.
// TODO: the actual database related traits should go into the `express-db` and `express-storage`
// crates.
#[async_trait]
pub trait ManageTaskStatus: Clone + Send + Sync + Sized {
    /// Get the status of duty associated with a particular [`OutputRef`].
    async fn get_status(&self, output_ref: OutputRef) -> Result<BridgeDutyStatus, TrackerError>;

    /// Update the checkpoint block height with new observed height.
    // TODO: the status should be an enum: `Received`, `Pending`, `Completed`.
    async fn update_status(
        &self,
        output_ref: OutputRef,
        status: BridgeDutyStatus,
    ) -> Result<(), TrackerError>;
}
