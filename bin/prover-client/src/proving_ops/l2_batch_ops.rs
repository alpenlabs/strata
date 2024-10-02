use std::sync::Arc;

use async_trait::async_trait;
use express_zkvm::Proof;
use uuid::Uuid;

use super::{cl_ops::ClOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::ProverInput,
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L2BatchOperations {
    cl_dispatcher: Arc<TaskDispatcher<ClOperations>>,
}

impl L2BatchOperations {
    /// Creates a new BTC operations instance.
    pub fn new(cl_dispatcher: Arc<TaskDispatcher<ClOperations>>) -> Self {
        Self { cl_dispatcher }
    }
}

#[derive(Debug, Clone)]
pub struct L2BatchInput {
    pub l2_range: (u64, u64),
    pub cl_task_ids: Vec<Uuid>,     // Task Ids of btc_ops tasks, in order
    pub proofs: Vec<Option<Proof>>, // Collected proofs from btc_ops tasks
}

#[async_trait]
impl ProvingOperations for L2BatchOperations {
    // Range of l1 blocks
    type Input = L2BatchInput;
    type Params = (u64, u64);

    fn block_type(&self) -> BlockType {
        BlockType::Btc
    }

    async fn fetch_input(&self, l2_range: (u64, u64)) -> Result<Self::Input, anyhow::Error> {
        // No additional fetching required
        let (start, end) = l2_range;
        let size = (end - start) as usize;
        let proofs = vec![None; size];
        let cl_task_ids = Vec::with_capacity(size);
        let input: Self::Input = L2BatchInput {
            l2_range,
            cl_task_ids,
            proofs,
        };
        Ok(input)
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        mut input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let mut dependencies = vec![];

        // Create CL tasks for each block in the l2 range
        let (start, end) = input.l2_range;
        for block_height in start..end {
            let cl_task_id = self
                .cl_dispatcher
                .create_task(block_height)
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(cl_task_id);
            input.cl_task_ids.push(cl_task_id);
        }

        // Create the l2_batch task with dependencies on CL tasks
        let task_id = task_tracker
            .create_task(ProverInput::L2Batch(input), dependencies)
            .await;
        Ok(task_id)
    }
}
