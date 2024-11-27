use std::sync::Arc;

use anyhow::anyhow;
use bitcoin::secp256k1::{SecretKey, SECP256K1};
use rand::{rngs::StdRng, SeedableRng};
use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_db::traits::{ProverDataStore, ProverDatabase};
use strata_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, ProofPublishMode, RollupParams, SyncParams},
    vk::{RollupVerifyingKey, StrataProofId},
};
use strata_proofimpl_btc_blockspace::{logic::BlockspaceProofInput, prover::BtcBlockspaceProver};
use strata_rocksdb::prover::db::ProverDB;
use uuid::Uuid;

use super::ProofGenerator;
use crate::{errors::ProvingTaskError, state::ProvingTask2, task2::TaskTracker2};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct BtcBlockspaceProofGenerator {
    btc_client: Arc<BitcoinClient>,
}

impl BtcBlockspaceProofGenerator {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>) -> Self {
        Self { btc_client }
    }
}

impl ProofGenerator for BtcBlockspaceProofGenerator {
    type Prover = BtcBlockspaceProver;

    async fn create_task(
        &self,
        id: &StrataProofId,
        db: &ProverDB,
        task_tracker: Arc<TaskTracker2>,
    ) -> Result<Uuid, ProvingTaskError> {
        let task = ProvingTask2::new(*id, vec![]);
        let task_id = task_tracker.insert_task(task).await;
        db.prover_store().insert_task(task_id, *id);
        Ok(task_id)
    }

    async fn fetch_input(
        &self,
        id: &StrataProofId,
        _db: &ProverDB,
    ) -> Result<<Self::Prover as strata_zkvm::ZkVmProver>::Input, anyhow::Error> {
        let height = match id {
            StrataProofId::BtcBlockspace(height) => height,
            _ => return Err(anyhow!("invalid input")),
        };
        let block = self.btc_client.get_block_at(*height).await?;
        let rollup_params = get_pm_rollup_params();
        Ok(BlockspaceProofInput {
            block,
            rollup_params,
        })
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
            genesis_l1_height: 5, // we have mainnet blocks from this height test-utils
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
    let mut rng = StdRng::seed_from_u64(seed);
    let sk = SecretKey::new(&mut rng);
    let (pk, _) = sk.x_only_public_key(SECP256K1);
    OperatorPubkeys::new(pk.into(), pk.into())
}
