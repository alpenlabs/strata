//! Defines traits related to managing the checkpoint for bridge duty executions and
//! some common implementers.

use async_trait::async_trait;

use super::errors::CheckpointError;

/// Defines functionalities to add, update and query checkpoints. This could be implemented by a
/// type like `FileCheckpointManager` or `DbCheckpointManager`.
#[async_trait]
pub trait ManageCheckpoint: Clone + Send + Sync + Sized {
    /// Get the checkpoint block height.
    async fn get_checkpoint(&self) -> Result<u64, CheckpointError>;

    /// Update the checkpoint block height with new observed height.
    async fn update_checkpoint(&self, block_height: u64) -> Result<(), CheckpointError>;
}
