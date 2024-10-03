use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use super::{btc_ops::BtcOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskType, ProvingTaskError},
    primitives::prover_input::{ProofWithVkey, ProverInput},
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L1BatchOperations {
    btc_dispatcher: Arc<TaskDispatcher<BtcOperations>>,
}

impl L1BatchOperations {
    /// Creates a new BTC operations instance.
    pub fn new(btc_dispatcher: Arc<TaskDispatcher<BtcOperations>>) -> Self {
        Self { btc_dispatcher }
    }
}

#[derive(Debug, Clone)]
pub struct L1BatchInput {
    pub l1_range: (u64, u64),
    pub btc_task_ids: Vec<Uuid>, // Task Ids of btc_ops tasks, in order
    pub proofs: Vec<Option<ProofWithVkey>>, // Collected proofs from btc_ops tasks
}

#[async_trait]
impl ProvingOperations for L1BatchOperations {
    // Range of l1 blocks
    type Input = L1BatchInput;
    type Params = (u64, u64);

    fn block_type(&self) -> ProvingTaskType {
        ProvingTaskType::Btc
    }

    async fn fetch_input(&self, l1_range: (u64, u64)) -> Result<Self::Input, anyhow::Error> {
        // No additional fetching required
        let (start, end) = l1_range;
        let size = (end - start) as usize;
        let proofs = vec![None; size];
        let btc_task_ids = Vec::with_capacity(size);
        let input: Self::Input = L1BatchInput {
            l1_range,
            btc_task_ids,
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

        // Create btc tasks for each block in the range
        let (start, end) = input.l1_range;
        for block_height in start..end {
            let btc_task_id = self
                .btc_dispatcher
                .create_task(block_height)
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(btc_task_id);
            input.btc_task_ids.push(btc_task_id);
        }

        // Create the l1_batch task with dependencies on btc tasks
        let task_id = task_tracker
            .create_task(ProverInput::L1Batch(input), dependencies)
            .await;
        Ok(task_id)
    }
}
