use std::sync::Arc;

use alloy_rpc_types::Block;
use async_trait::async_trait;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_proofimpl_evm_ee_stf::ELProofInput;
use tracing::debug;
use uuid::Uuid;

use super::ops::ProvingOperations;
use crate::{
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{WitnessData, ZkVmInput},
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
    type Params = (u64, u64);

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::EL
    }

    async fn fetch_input(&self, block_range: Self::Params) -> Result<Self::Input, anyhow::Error> {
        let (start_block_num, end_block_num) = block_range;
        let mut el_proof_inputs: Vec<ELProofInput> = Vec::new();

        for block_num in start_block_num..=end_block_num {
            let block: Block = self
                .el_client
                .request(
                    "eth_getBlockByNumber",
                    rpc_params![format!("0x{:x}", block_num), false],
                )
                .await?;
            let block_hash = block.header.hash;
            let el_proof_input: ELProofInput = self
                .el_client
                .request("strataee_getBlockWitness", rpc_params![block_hash, true])
                .await?;

            el_proof_inputs.push(el_proof_input);
        }

        debug!("Fetched EL block witness for block {:?}", block_range);
        Ok(bincode::serialize(&el_proof_inputs).unwrap())
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let data = WitnessData { data: input };
        let prover_input = ZkVmInput::ElBlock(data);
        let task_id = task_tracker.create_task(prover_input, vec![]).await;
        Ok(task_id)
    }
}
