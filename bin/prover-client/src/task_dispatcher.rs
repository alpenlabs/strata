use std::sync::Arc;

use anyhow::Context;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use reth_rpc_types::Block;
use tracing::error;
use uuid::Uuid;
use zkvm_primitives::ZKVMInput;

use crate::{
    primitives::prover_input::{ProverInput, WitnessData},
    task_tracker::TaskTracker,
};

#[derive(Clone)]
pub struct ELBlockProvingTaskScheduler {
    el_rpc_client: HttpClient,
    task_tracker: Arc<TaskTracker>,
    last_block_sent: u64,
}

impl ELBlockProvingTaskScheduler {
    pub fn new(el_rpc_client: HttpClient, task_tracker: Arc<TaskTracker>) -> Self {
        Self {
            el_rpc_client,
            task_tracker,
            last_block_sent: 0,
        }
    }

    // Start listening for new blocks and process them automatically
    pub async fn listen_for_new_blocks(&mut self) {
        loop {
            let next_block = self.last_block_sent + 1;
            if let Err(e) = self.create_proving_task(next_block).await {
                error!("Error processing block: {:?}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            } else {
                self.last_block_sent = next_block;
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    // Create proving task for the given block idx
    pub async fn create_proving_task(&self, block_num: u64) -> anyhow::Result<Uuid> {
        let prover_input = self.fetch_el_block_prover_input(block_num).await?;
        self.append_proving_task(prover_input, block_num).await
    }

    // Append the proving task to the task tracker
    async fn append_proving_task(
        &self,
        prover_input: ZKVMInput,
        block_num: u64,
    ) -> anyhow::Result<Uuid> {
        let el_block_witness = WitnessData {
            data: bincode::serialize(&prover_input)?,
        };
        let witness = ProverInput::ElBlock(el_block_witness);
        let task_id = self.task_tracker.create_task(block_num, witness).await;
        Ok(task_id)
    }

    // Fetch EL block prover input from the RPC client
    async fn fetch_el_block_prover_input(&self, el_block_num: u64) -> anyhow::Result<ZKVMInput> {
        let el_block: Block = self
            .el_rpc_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", el_block_num), false],
            )
            .await
            .context("Failed to get the el block")?;

        let el_block_witness: ZKVMInput = self
            .el_rpc_client
            .request(
                "alpee_getBlockWitness",
                rpc_params![el_block.header.hash.context("Block hash missing")?, true],
            )
            .await
            .context("Failed to get the EL witness")?;

        Ok(el_block_witness)
    }
}
