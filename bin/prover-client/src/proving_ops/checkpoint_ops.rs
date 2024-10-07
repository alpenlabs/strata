use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_rpc_types::RpcCheckpointInfo;
use tracing::debug;
use uuid::Uuid;

use super::{
    l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations, ops::ProvingOperations,
};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZKVMInput},
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct CheckpointOperations {
    cl_client: HttpClient,
    l1_batch_dispatcher: Arc<TaskDispatcher<L1BatchOperations>>,
    l2_batch_dispatcher: Arc<TaskDispatcher<L2BatchOperations>>,
}

impl CheckpointOperations {
    /// Creates a new BTC operations instance.
    pub fn new(
        cl_client: HttpClient,
        l1_batch_dispatcher: Arc<TaskDispatcher<L1BatchOperations>>,
        l2_batch_dispatcher: Arc<TaskDispatcher<L2BatchOperations>>,
    ) -> Self {
        Self {
            cl_client,
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
    pub l1_batch_proof: Option<ProofWithVkey>,
    pub l2_batch_proof: Option<ProofWithVkey>,
}

#[derive(Debug, Clone)]
pub enum CheckpointOpsParam {
    Latest,
    Manual(RpcCheckpointInfo),
    CheckPointIndex(u64),
}

#[async_trait]
impl ProvingOperations for CheckpointOperations {
    // Range of l1 blocks
    type Input = CheckpointInput;
    type Params = CheckpointOpsParam;

    fn block_type(&self) -> ProvingTaskType {
        ProvingTaskType::Btc
    }

    async fn fetch_input(&self, info: CheckpointOpsParam) -> Result<Self::Input, anyhow::Error> {
        let rpc_ckp_info = match info {
            CheckpointOpsParam::Latest => {
                debug!("Fetching latest checkpoint from the sequencer");
                let ckp_idx = self
                    .cl_client
                    .request::<Option<u64>, _>("strata_getLatestCheckpointIndex", rpc_params![])
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Checkpoint information not found"))?;

                self.cl_client
                    .request::<Option<RpcCheckpointInfo>, _>(
                        "strata_getCheckpointInfo",
                        rpc_params![ckp_idx],
                    )
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("Checkpoint information not found for index {}", ckp_idx)
                    })?
            }
            CheckpointOpsParam::CheckPointIndex(ckp_idx) => {
                let mut res = self
                    .cl_client
                    .request::<Option<RpcCheckpointInfo>, _>(
                        "strata_getCheckpointInfo",
                        rpc_params![ckp_idx],
                    )
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("Checkpoint information not found for index {}", ckp_idx)
                    })?;

                // TODO: TempFix
                res.l1_range = (1, 1);
                res
            }
            CheckpointOpsParam::Manual(ckp_info) => ckp_info,
        };

        Ok(CheckpointInput {
            info: rpc_ckp_info,
            l1_batch_id: Uuid::nil(),
            l2_batch_id: Uuid::nil(),
            l1_batch_proof: None,
            l2_batch_proof: None,
        })
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
                ZKVMInput::Checkpoint(input),
                vec![l1_batch_task_id, l2_batch_task_id],
            )
            .await;
        Ok(task_id)
    }
}
