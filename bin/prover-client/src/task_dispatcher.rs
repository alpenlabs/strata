use std::sync::Arc;

use anyhow::Context;
use express_proofimpl_evm_ee_stf::ELProofInput;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use reth_rpc_types::Block;
use tracing::error;
use uuid::Uuid;

use crate::{
    config::BLOCK_PROVING_TASK_DISPATCH_INTERVAL,
    primitives::prover_input::{ProverInput, WitnessData},
    task_tracker::TaskTracker,
};

/// The `ELBlockProvingTaskScheduler` handles the scheduling of EL block proving tasks.
/// It listens for new EL blocks via an RPC client, fetches the necessary proving inputs,
/// and adds these tasks to a shared `TaskTracker` for further processing.
#[derive(Clone)]
pub struct ELBlockProvingTaskScheduler {
    el_rpc_client: HttpClient,
    task_tracker: Arc<TaskTracker>,
    last_block_sent: u64,
}

impl ELBlockProvingTaskScheduler {
    pub fn new(
        el_rpc_client: HttpClient,
        task_tracker: Arc<TaskTracker>,
        start_block_height: u64,
    ) -> Self {
        Self {
            el_rpc_client,
            task_tracker,
            last_block_sent: start_block_height,
        }
    }

    // Start listening for new blocks and process them automatically
    pub async fn listen_for_new_blocks(&mut self) {
        loop {
            if let Err(e) = self.create_proving_task(self.last_block_sent).await {
                error!("Error processing block: {:?}", e);
            } else {
                self.last_block_sent += 1;
            }

            tokio::time::sleep(std::time::Duration::from_secs(
                BLOCK_PROVING_TASK_DISPATCH_INTERVAL,
            ))
            .await;
        }
    }

    // Create proving task for the given block idx
    pub async fn create_proving_task(&self, block_num: u64) -> anyhow::Result<Uuid> {
        let prover_input = self.fetch_el_block_prover_input(block_num).await?;
        self.append_proving_task(prover_input).await
    }

    // Append the proving task to the task tracker
    async fn append_proving_task(&self, prover_input: ELProofInput) -> anyhow::Result<Uuid> {
        let el_block_witness = WitnessData {
            data: bincode::serialize(&prover_input)?,
        };
        let witness = ProverInput::ElBlock(el_block_witness);
        let task_id = self.task_tracker.create_task(witness).await;
        Ok(task_id)
    }

    // Fetch EL block prover input from the RPC client
    async fn fetch_el_block_prover_input(&self, el_block_num: u64) -> anyhow::Result<ELProofInput> {
        let el_block: Block = self
            .el_rpc_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", el_block_num), false],
            )
            .await
            .context("Failed to get the el block")?;

        let el_block_witness: ELProofInput = self
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
