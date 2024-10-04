use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use express_proofimpl_evm_ee_stf::ELProofInput;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use reth_rpc_types::Block;
use tracing::debug;
use uuid::Uuid;

use super::ops::ProvingOperations;
use crate::{
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{WitnessData, ZKVMInput},
    task::TaskTracker,
};

/// Operations required for EL block proving tasks.
#[derive(Debug, Clone)]
pub struct ElOperations {
    el_client: HttpClient,
}

impl ElOperations {
    /// Creates a new EL operations instance.
    pub fn new(el_client: HttpClient) -> Self {
        Self { el_client }
    }
}

#[async_trait]
impl ProvingOperations for ElOperations {
    /// Used serialized [`ELProofInput`] because [`ELProofInput::parent_state_trie`] contains
    /// RefCell, which is not Sync or Send
    type Input = Vec<u8>;
    type Params = u64;

    fn block_type(&self) -> ProvingTaskType {
        ProvingTaskType::EL
    }

    async fn fetch_input(&self, block_num: u64) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching EL block input");
        let block: Block = self
            .el_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", block_num), false],
            )
            .await?;
        let block_hash = block.header.hash.context("Block hash missing")?;
        let witness: ELProofInput = self
            .el_client
            .request("alpee_getBlockWitness", rpc_params![block_hash, true])
            .await?;
        debug!("Fetched EL block witness for block {}", block_num);
        Ok(bincode::serialize(&witness).unwrap())
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let data = WitnessData { data: input };
        let prover_input = ZKVMInput::ElBlock(data);
        let task_id = task_tracker.create_task(prover_input, vec![]).await;
        Ok(task_id)
    }
}
