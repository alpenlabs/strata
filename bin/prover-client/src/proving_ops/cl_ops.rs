use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tracing::debug;
use uuid::Uuid;

use super::{el_ops::ElOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZkVmInput},
    task::TaskTracker,
};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClOperations {
    cl_client: HttpClient,
    el_dispatcher: Arc<TaskDispatcher<ElOperations>>,
}

impl ClOperations {
    /// Creates a new CL operations instance.
    pub fn new(cl_client: HttpClient, el_dispatcher: Arc<TaskDispatcher<ElOperations>>) -> Self {
        Self {
            cl_client,
            el_dispatcher,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CLProverInput {
    pub blocks: (u64, u64),
    pub raw_witness: Vec<Vec<u8>>,
    pub ee_proof: Option<ProofWithVkey>,
}

#[async_trait]
impl ProvingOperations for ClOperations {
    type Input = CLProverInput;
    type Params = (u64, u64);

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::CL
    }

    async fn fetch_input(&self, block_range: Self::Params) -> Result<Self::Input, anyhow::Error> {
        let (start_block_num, end_block_num) = block_range;
        let mut cl_proof_inputs: Vec<Vec<u8>> = Vec::new();

        for block_num in start_block_num..=end_block_num {
            let witness: Option<Vec<u8>> = self
                .cl_client
                .request("strata_getCLBlockWitness", rpc_params![block_num])
                .await
                .unwrap();
            let cl_raw_witness = witness.context("Failed to get the CL witness")?;
            cl_proof_inputs.push(cl_raw_witness);
        }

        debug!("Fetched CL block witness for block {:?}", block_range);
        Ok(CLProverInput {
            blocks: block_range,
            raw_witness: cl_proof_inputs,
            ee_proof: None,
        })
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        // Create proving task for the corresponding EL block
        let el_task_id = self.el_dispatcher.create_task(input.blocks).await?;
        let prover_input = ZkVmInput::ClBlock(input);

        let task_id = task_tracker
            .create_task(prover_input, vec![el_task_id])
            .await;

        Ok(task_id)
    }
}
