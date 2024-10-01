use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tracing::debug;
use uuid::Uuid;

use super::ops::ProvingOperations;
use crate::{
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::{ProverInput, WitnessData},
    task::TaskTracker,
};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClOperations {
    cl_client: HttpClient,
}

impl ClOperations {
    /// Creates a new CL operations instance.
    pub fn new(cl_client: HttpClient) -> Self {
        Self { cl_client }
    }
}

#[async_trait]
impl ProvingOperations for ClOperations {
    type Input = Vec<u8>;

    fn block_type(&self) -> BlockType {
        BlockType::CL
    }

    async fn fetch_input(&self, block_num: u64) -> Result<Self::Input, anyhow::Error> {
        debug!("Fetching CL block input for block {}", block_num);
        let witness: Option<Vec<u8>> = self
            .cl_client
            .request("alp_getCLBlockWitness", rpc_params![block_num])
            .await?;
        witness.context("Failed to get the CL witness")
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let data = WitnessData { data: input };
        let prover_input = ProverInput::ClBlock(data);
        let task_id = task_tracker.create_task(prover_input).await;
        Ok(task_id)
    }
}
