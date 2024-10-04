use std::sync::Arc;

use anyhow::Context;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use reth_rpc_types::Block;
use strata_proofimpl_evm_ee_stf::ELProofInput;
use tokio::time::{self, Duration};
use tracing::error;
use uuid::Uuid;

use crate::{
    config::BLOCK_PROVING_TASK_DISPATCH_INTERVAL,
    errors::ELProvingTaskError,
    primitives::prover_input::{ProverInput, WitnessData},
    task::TaskTracker,
};

/// The `ELBlockProvingTaskScheduler` handles the scheduling of EL block proving tasks.
/// It listens for new EL blocks via an RPC client, fetches the necessary proving inputs,
/// and adds these tasks to a shared `TaskTracker` for further processing.
#[derive(Clone)]
pub struct ELBlockProvingTaskScheduler {
    /// The RPC client used to communicate with the EL network.
    /// It listens for new EL blocks and retrieves necessary data for proving.
    el_rpc_client: HttpClient,

    /// A shared `TaskTracker` instance. It tracks and manages the lifecycle of proving tasks added
    /// by the scheduler.
    task_tracker: Arc<TaskTracker>,

    /// Stores the identifier of the last EL block that was sent for proving.
    /// This helps in tracking progress and avoiding duplicate task submissions.
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
        let mut interval =
            time::interval(Duration::from_secs(BLOCK_PROVING_TASK_DISPATCH_INTERVAL));

        loop {
            match self.create_proving_task(self.last_block_sent).await {
                Ok(_) => {
                    self.last_block_sent += 1;
                }
                Err(e) => {
                    error!("Error processing block {}: {:?}", self.last_block_sent, e);
                }
            }

            interval.tick().await;
        }
    }

    // Create proving task for the given block idx
    pub async fn create_proving_task(&self, block_num: u64) -> Result<Uuid, ELProvingTaskError> {
        let prover_input = self
            .fetch_el_block_prover_input(block_num)
            .await
            .map_err(|e| ELProvingTaskError::FetchElBlockProverInputError {
                block_num,
                source: e,
            })?;

        self.append_proving_task(prover_input).await
    }

    // Append the proving task to the task tracker
    async fn append_proving_task(
        &self,
        prover_input: ELProofInput,
    ) -> Result<Uuid, ELProvingTaskError> {
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
                "strataee_getBlockWitness",
                rpc_params![el_block.header.hash.context("Block hash missing")?, true],
            )
            .await
            .context("Failed to get the EL witness")?;

        Ok(el_block_witness)
    }
}
