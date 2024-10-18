use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::Block;
use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::{block_credential::CredRule, params::RollupParams};
use strata_tx_parser::filter::{derive_tx_filter_rules, TxFilterRule};
use tracing::debug;
use uuid::Uuid;

use super::ops::ProvingOperations;
use crate::{
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::ZKVMInput,
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct BtcOperations {
    rollup_params: Arc<RollupParams>,
    btc_client: Arc<BitcoinClient>,
}

impl BtcOperations {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>, rollup_params: Arc<RollupParams>) -> Self {
        Self {
            btc_client,
            rollup_params,
        }
    }
}

#[async_trait]
impl ProvingOperations for BtcOperations {
    type Input = (Block, Vec<TxFilterRule>, CredRule);
    type Params = u64; // params is the block height

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::Btc
    }

    async fn fetch_input(&self, block_num: Self::Params) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching BTC block input");
        let block = self.btc_client.get_block_at(block_num).await?;
        debug!("Fetched BTC block {}", block_num);
        let tx_filters = derive_tx_filter_rules(&self.rollup_params)?;
        let cred_rule = &self.rollup_params.cred_rule;
        Ok((block, tx_filters, cred_rule.clone()))
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let (block, tx_filters, cred_rule) = input;
        let prover_input = ZKVMInput::BtcBlock(block, cred_rule, tx_filters);
        let task_id = task_tracker.create_task(prover_input, vec![]).await;
        Ok(task_id)
    }
}
