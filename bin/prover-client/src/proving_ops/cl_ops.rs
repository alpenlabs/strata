use std::sync::Arc;

use alpen_express_state::{block::L2Block, chain_state::ChainState};
use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tracing::debug;
use uuid::Uuid;

use super::{el_ops::ElOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::ProverInput,
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

#[async_trait]
impl ProvingOperations for ClOperations {
    type Input = (ChainState, L2Block);
    type Params = u64;

    fn block_type(&self) -> BlockType {
        BlockType::CL
    }

    async fn fetch_input(&self, block_num: u64) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching CL block input");
        let witness: Option<Vec<u8>> = self
            .cl_client
            .request("alp_getCLBlockWitness", rpc_params![block_num])
            .await?;
        let witness = witness.context("Failed to get the CL witness")?;
        let (chain_state, l2_blk_bundle): (ChainState, L2Block) =
            borsh::from_slice(&witness).map_err(|e| ProvingTaskError::Serialization(e.into()))?;
        Ok((chain_state, l2_blk_bundle))
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let (chain_state, l2_block) = input;

        let el_task_id = self
            .el_dispatcher
            .create_task(chain_state.chain_tip_slot())
            .await?;

        let prover_input = ProverInput::ClBlock(chain_state, l2_block);

        let task_id = task_tracker
            .create_task(prover_input, vec![el_task_id])
            .await;

        Ok(task_id)
    }
}
