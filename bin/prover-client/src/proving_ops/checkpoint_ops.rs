use std::sync::Arc;

use alpen_express_rpc_types::RpcCheckpointInfo;
use async_trait::async_trait;
use express_zkvm::Proof;
use uuid::Uuid;

use super::{
    l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations, ops::ProvingOperations,
};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::ProverInput,
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct CheckpointOperations {
    l1_batch_dispatcher: Arc<TaskDispatcher<L1BatchOperations>>,
    l2_batch_dispatcher: Arc<TaskDispatcher<L2BatchOperations>>,
}

impl CheckpointOperations {
    /// Creates a new BTC operations instance.
    pub fn new(
        l1_batch_dispatcher: Arc<TaskDispatcher<L1BatchOperations>>,
        l2_batch_dispatcher: Arc<TaskDispatcher<L2BatchOperations>>,
    ) -> Self {
        Self {
            l1_batch_dispatcher,
            l2_batch_dispatcher,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointInput {
    pub info: RpcCheckpointInfo,
    pub l1_batch_id: Uuid,
    pub l2_batch_id: Uuid,
    pub l1_batch_proof: Option<Proof>,
    pub l2_batch_proof: Option<Proof>,
}

#[async_trait]
impl ProvingOperations for CheckpointOperations {
    // Range of l1 blocks
    type Input = CheckpointInput;
    type Params = RpcCheckpointInfo;

    fn block_type(&self) -> BlockType {
        BlockType::Btc
    }

    async fn fetch_input(&self, info: RpcCheckpointInfo) -> Result<Self::Input, anyhow::Error> {
        let input: Self::Input = CheckpointInput {
            info,
            l1_batch_id: Uuid::nil(),
            l2_batch_id: Uuid::nil(),
            l1_batch_proof: None,
            l2_batch_proof: None,
        };
        Ok(input)
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        mut input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let l1_batch_task_id = self
            .l1_batch_dispatcher
            .create_task(input.info.l1_range)
            .await
            .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
        input.l1_batch_id = l1_batch_task_id;

        let l2_batch_task_id = self
            .l2_batch_dispatcher
            .create_task(input.info.l2_range)
            .await
            .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
        input.l2_batch_id = l2_batch_task_id;

        // Create the checkpoitn task with dependencies on both l1_batch and l2_batch
        let task_id = task_tracker
            .create_task(
                ProverInput::Checkpoint(input),
                vec![l1_batch_task_id, l2_batch_task_id],
            )
            .await;
        Ok(task_id)
    }
}
