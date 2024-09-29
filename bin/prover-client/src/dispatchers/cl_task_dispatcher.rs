use std::sync::Arc;

use anyhow::Context;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tokio::time::{self, Duration};
use tracing::error;
use uuid::Uuid;

use crate::{
    config::BLOCK_PROVING_TASK_DISPATCH_INTERVAL,
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::{ProverInput, WitnessData},
    task::TaskTracker,
};

/// The `CLBlockProvingTaskDispatcher` handles the dispatching of CL block proving tasks to the
/// TaskTracker. It listens for new CL blocks via an RPC client, fetches the necessary proving
/// inputs, and adds these tasks to a shared `TaskTracker` for further processing.
#[derive(Clone)]
pub struct CLBlockProvingTaskDispatcher {
    /// The RPC client used to communicate with the CL network.
    /// It listens for new CL blocks and retrieves necessary data for proving.
    sequnecer_rpc_client: HttpClient,

    /// A shared `TaskTracker` instance. It tracks and manages the lifecycle of proving tasks added
    /// by the scheduler.
    task_tracker: Arc<TaskTracker>,

    /// Stores the identifier of the last CL block that was sent for proving.
    /// This helps in tracking progress and avoiding duplicate task submissions.
    last_block_sent: u64,
}

impl CLBlockProvingTaskDispatcher {
    pub fn new(
        sequencer_rpc_client: HttpClient,
        task_tracker: Arc<TaskTracker>,
        start_block_height: u64,
    ) -> Self {
        Self {
            sequnecer_rpc_client: sequencer_rpc_client,
            task_tracker,
            last_block_sent: start_block_height,
        }
    }

    pub fn task_tracker(&self) -> &Arc<TaskTracker> {
        &self.task_tracker
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
    pub async fn create_proving_task(&self, block_num: u64) -> Result<Uuid, ProvingTaskError> {
        if let Some(raw_witness) =
            self.fetch_cl_block_prover_input(block_num)
                .await
                .map_err(|e| ProvingTaskError::FetchBlockProverInputError {
                    block_num,
                    task_type: BlockType::CL,
                    source: e,
                })?
        {
            let file_name = format!("cl_block_{:?}", block_num);
            use std::{fs::File, io::Write};
            let mut file = File::create(file_name).unwrap();
            file.write_all(&raw_witness).unwrap();

            return self.append_proving_task(raw_witness).await;
        }

        Err(ProvingTaskError::FetchBlockProverInputError {
            block_num,
            task_type: BlockType::CL,
            source: anyhow::anyhow!(
                "Failed to find the raw witness for the CL block {:?}",
                block_num
            ),
        })
    }

    // Append the proving task to the task tracker
    async fn append_proving_task(&self, prover_input: Vec<u8>) -> Result<Uuid, ProvingTaskError> {
        let el_block_witness = WitnessData {
            data: bincode::serialize(&prover_input)?,
        };
        let witness = ProverInput::ClBlock(el_block_witness);
        let task_id = self.task_tracker.create_task(witness).await;
        Ok(task_id)
    }

    // Fetch CL block prover input from the RPC client
    async fn fetch_cl_block_prover_input(
        &self,
        cl_block_num: u64,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let cl_block_witness: Option<Vec<u8>> = self
            .sequnecer_rpc_client
            .request("alp_getCLBlockWitness", rpc_params![cl_block_num])
            .await
            .context("Failed to get the CL witness")?;

        Ok(cl_block_witness)
    }
}
