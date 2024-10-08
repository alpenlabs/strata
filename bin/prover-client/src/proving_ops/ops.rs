use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    errors::{ProvingTaskError, ProvingTaskType},
    task::TaskTracker,
};

/// Trait defining operations required for block proving tasks.
#[async_trait]
pub trait ProvingOperations: Send + Sync {
    type Input: Send + Sync;
    type Params: Send + Sync;

    /// Returns the block type (e.g., BTC, EL, CL).
    fn proving_task_type(&self) -> ProvingTaskType;

    /// Fetches the prover input for the given block number.
    async fn fetch_input(&self, params: Self::Params) -> Result<Self::Input, anyhow::Error>;

    /// Appends a proving task to the task tracker.
    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError>;
}
