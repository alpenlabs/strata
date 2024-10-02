use std::sync::Arc;

use alpen_express_btcio::rpc::{traits::Reader, BitcoinClient};
use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, ProofPublishMode, RollupParams},
    vk::RollupVerifyingKey,
};
use async_trait::async_trait;
use bitcoin::Block;
use strata_tx_parser::filter::TxFilterRule;
use tracing::debug;
use uuid::Uuid;

use super::ops::ProvingOperations;
use crate::{
    errors::{BlockType, ProvingTaskError},
    primitives::prover_input::ProverInput,
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct BtcOperations {
    btc_client: Arc<BitcoinClient>,
}

impl BtcOperations {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>) -> Self {
        Self { btc_client }
    }
}

#[async_trait]
impl ProvingOperations for BtcOperations {
    type Input = (Block, Vec<TxFilterRule>);
    type Params = u64; // params is the block height

    fn block_type(&self) -> BlockType {
        BlockType::Btc
    }

    async fn fetch_input(&self, block_num: u64) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching BTC block input");
        let filters = get_tx_filters();
        let block = self.btc_client.get_block_at(block_num).await?;
        debug!("Fetched BTC block {}", block_num);
        Ok((block, filters))
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let (block, filters) = input;
        let prover_input = ProverInput::BtcBlock(block, filters);
        let task_id = task_tracker.create_task(prover_input, vec![]).await;
        Ok(task_id)
    }
}

/// Generates transaction filters for BTC blocks.
fn get_tx_filters() -> Vec<TxFilterRule> {
    let rollup_params = default_rollup_params();
    let rollup_name = rollup_params.rollup_name.clone();
    // let deposit_config = DepositTxConfig::from_rollup_params(&rollup_params);
    vec![
        // TODO:
        // TxFilterRule::Deposit(deposit_config),
        TxFilterRule::RollupInscription(rollup_name),
    ]
}

fn default_rollup_params() -> RollupParams {
    // FIXME this is broken, where are the keys?
    let opkeys = OperatorPubkeys::new(Buf32::zero(), Buf32::zero());

    // TODO: load default params from a json during compile time
    RollupParams {
        rollup_name: "express".to_string(),
        block_time: 1000,
        cred_rule: block_credential::CredRule::Unchecked,
        horizon_l1_height: 3,
        genesis_l1_height: 5,
        operator_config: OperatorConfig::Static(vec![opkeys]),
        evm_genesis_block_hash: Buf32(
            "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                .parse()
                .unwrap(),
        ),
        evm_genesis_block_state_root: Buf32(
            "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                .parse()
                .unwrap(),
        ),
        l1_reorg_safe_depth: 4,
        target_l2_batch_size: 64,
        address_length: 20,
        deposit_amount: 1_000_000_000,
        rollup_vk: RollupVerifyingKey::SP1VerifyingKey(Buf32(
            "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
                .parse()
                .unwrap(),
        )), // TODO: update this with vk for checkpoint proof
        verify_proofs: true,
        dispatch_assignment_dur: 64,
        proof_publish_mode: ProofPublishMode::Strict,
        max_deposits_in_block: 16,
    }
}
