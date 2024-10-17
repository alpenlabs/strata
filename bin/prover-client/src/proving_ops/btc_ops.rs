use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::{key::Secp256k1, Block};
use musig2::secp256k1::SecretKey;
use rand::{rngs::StdRng, SeedableRng};
use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::{
    block_credential::{self, CredRule},
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, ProofPublishMode, RollupParams, SyncParams},
    vk::RollupVerifyingKey,
};
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
    type Input = (Block, Vec<TxFilterRule>, CredRule);
    type Params = u64; // params is the block height

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::Btc
    }

    async fn fetch_input(&self, block_num: Self::Params) -> Result<Self::Input, anyhow::Error> {
        debug!(%block_num, "Fetching BTC block input");
        let block = self.btc_client.get_block_at(block_num).await?;
        debug!("Fetched BTC block {}", block_num);
        let tx_filters = derive_tx_filter_rules(&get_pm_rollup_params())?;
        let cred_rule = get_pm_rollup_params().cred_rule;
        Ok((block, tx_filters, cred_rule))
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

// TODO: Move from manual param generation to importing params from the file
pub fn get_pm_rollup_params() -> RollupParams {
    // TODO: create a random seed if we really need random op_pubkeys every time this is called
    gen_params_with_seed(0).rollup
}

fn gen_params_with_seed(seed: u64) -> Params {
    let opkeys = make_dummy_operator_pubkeys_with_seed(seed);
    Params {
        rollup: RollupParams {
            rollup_name: "strata".to_string(),
            block_time: 1000,
            cred_rule: block_credential::CredRule::Unchecked,
            horizon_l1_height: 3,
            genesis_l1_height: 500, // we have mainnet blocks from this height test-utils
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
                "0x00d952e267176e21519a9a00aeb2c54d08fe6c9928fcea2173a8ef3eb60b6681"
                    .parse()
                    .unwrap(),
            )),
            dispatch_assignment_dur: 64,
            proof_publish_mode: ProofPublishMode::Strict,
            max_deposits_in_block: 16,
            network: bitcoin::Network::Regtest,
        },
        run: SyncParams {
            l2_blocks_fetch_limit: 1000,
            l1_follow_distance: 3,
            client_checkpoint_interval: 10,
        },
    }
}

pub fn make_dummy_operator_pubkeys_with_seed(seed: u64) -> OperatorPubkeys {
    let secp = Secp256k1::new();
    let mut rng = StdRng::seed_from_u64(seed);
    let sk = SecretKey::new(&mut rng);
    let (pk, _) = sk.x_only_public_key(&secp);
    OperatorPubkeys::new(pk.into(), pk.into())
}

#[cfg(test)]
mod test {
    use super::gen_params_with_seed;

    #[test]
    fn simple() {
        let params = gen_params_with_seed(0);
        println!("got the params {:#?}", params);

        let params_hash = params.rollup().compute_hash();
        println!("got the rollup params hash {:#?}", params_hash)
    }
}
