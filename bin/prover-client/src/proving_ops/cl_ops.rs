use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::params::RollupParams;
use tracing::debug;
use uuid::Uuid;

use super::{el_ops::ElOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZKVMInput},
    task::TaskTracker,
};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClOperations {
    cl_client: HttpClient,
    el_dispatcher: Arc<TaskDispatcher<ElOperations>>,
    rollup_params: Arc<RollupParams>,
}

impl ClOperations {
    /// Creates a new CL operations instance.
    pub fn new(
        cl_client: HttpClient,
        el_dispatcher: Arc<TaskDispatcher<ElOperations>>,
        rollup_params: Arc<RollupParams>,
    ) -> Self {
        Self {
            cl_client,
            el_dispatcher,
            rollup_params,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CLProverInput {
    pub block_num: u64,
    pub cl_raw_witness: Vec<u8>,
    pub rollup_params: RollupParams,
    pub el_proof: Option<ProofWithVkey>,
}

#[async_trait]
impl ProvingOperations for ClOperations {
    type Input = CLProverInput;
    type Params = u64;

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::CL
    }

    async fn fetch_input(&self, block_num: Self::Params) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching CL block input");
        let witness: Option<Vec<u8>> = self
            .cl_client
            .request("strata_getCLBlockWitness", rpc_params![block_num])
            .await
            .unwrap();
        let cl_raw_witness = witness.context("Failed to get the CL witness")?;

        Ok(CLProverInput {
            block_num,
            cl_raw_witness,
            el_proof: None,
            rollup_params: (*self.rollup_params).clone(),
        })
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let el_task_id = self.el_dispatcher.create_task(input.block_num).await?;

        let prover_input = ZKVMInput::ClBlock(input);

        let task_id = task_tracker
            .create_task(prover_input, vec![el_task_id])
            .await;

        Ok(task_id)
    }
}
