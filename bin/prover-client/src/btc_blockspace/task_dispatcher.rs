use std::sync::Arc;

use alpen_express_btcio::rpc::{traits::Reader, BitcoinClient};
use alpen_express_primitives::{buf::Buf32, l1::XOnlyPk};
use bitcoin::Block;
use strata_tx_parser::{deposit::DepositTxConfig, filter::TxFilterRule};
use tokio::time::{self, Duration};
use tracing::{debug, error};
use uuid::Uuid;

use super::errors::BtcProvingTaskError;
use crate::{config::BTC_BLOCK_TIME, primitives::prover_input::ProverInput, task::TaskTracker};

/// The `ELBlockProvingTaskScheduler` handles the scheduling of EL block proving tasks.
/// It listens for new EL blocks via an RPC client, fetches the necessary proving inputs,
/// and adds these tasks to a shared `TaskTracker` for further processing.
#[derive(Clone)]
pub struct BtcBlockspaceProvingTaskScheduler {
    /// The RPC client used to communicate with the BTC network.
    /// It listens for new BTC blocks and retrieves necessary data for proving.
    btc_client: Arc<BitcoinClient>,

    /// A shared `TaskTracker` instance. It tracks and manages the lifecycle of proving tasks added
    /// by the scheduler.
    task_tracker: Arc<TaskTracker>,

    /// Stores the identifier of the last BTC block that was sent for proving.
    /// This helps in tracking progress and avoiding duplicate task submissions.
    last_block_sent: u64,
}

impl BtcBlockspaceProvingTaskScheduler {
    pub fn new(
        btc_client: Arc<BitcoinClient>,
        task_tracker: Arc<TaskTracker>,
        start_block_height: u64,
    ) -> Self {
        Self {
            btc_client,
            task_tracker,
            last_block_sent: start_block_height,
        }
    }

    // Start listening for new blocks and process them automatically
    pub async fn listen_for_new_blocks(&mut self) {
        let mut interval = time::interval(Duration::from_secs(BTC_BLOCK_TIME));

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
    pub async fn create_proving_task(&self, block_num: u64) -> Result<Uuid, BtcProvingTaskError> {
        debug!("try to create a Bitcoin proving task");
        let (block, filters) = self
            .fetch_btc_block_prover_input(block_num)
            .await
            .map_err(|e| BtcProvingTaskError::FetchBtcBlockProverInputError {
                block_num,
                source: e,
            })?;

        self.append_proving_task(block, filters).await
    }

    // Append the proving task to the task tracker
    async fn append_proving_task(
        &self,
        block: Block,
        filters: Vec<TxFilterRule>,
    ) -> Result<Uuid, BtcProvingTaskError> {
        let witness = ProverInput::BtcBlock(block, filters);
        let task_id = self.task_tracker.create_task(witness).await;
        Ok(task_id)
    }

    // Fetch BTC block
    async fn fetch_btc_block_prover_input(
        &self,
        btc_block_num: u64,
    ) -> anyhow::Result<(Block, Vec<TxFilterRule>)> {
        tracing::debug!("Fetching btc block prover input");
        let tx_filters = get_tx_filters();
        let block = self.btc_client.get_block_at(btc_block_num).await.unwrap();
        tracing::debug!("Fetched btc block");
        Ok((block, tx_filters))
    }
}

// TODO: get this from strata endpoint
fn get_tx_filters() -> Vec<TxFilterRule> {
    let agg_addr = XOnlyPk::new(Buf32::zero());
    let addr_len = 20;
    let rollup_name = "strata".to_owned();
    let magic_bytes = rollup_name.clone().into_bytes().to_vec();
    let deposit_tx_config = DepositTxConfig::new(&magic_bytes, addr_len, 100_000, agg_addr);

    vec![
        TxFilterRule::Deposit(deposit_tx_config),
        TxFilterRule::RollupInscription(rollup_name),
    ]
}
