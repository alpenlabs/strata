use std::sync::Arc;

use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_btc_blockspace::{logic::BlockspaceProofInput, prover::BtcBlockspaceProver};
use strata_rocksdb::prover::db::ProofDb;
use tokio::sync::Mutex;

use super::{utils::get_pm_rollup_params, ProvingOp};
use crate::{errors::ProvingTaskError, task::TaskTracker};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct BtcBlockspaceHandler {
    btc_client: Arc<BitcoinClient>,
}

impl BtcBlockspaceHandler {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>) -> Self {
        Self { btc_client }
    }
}

impl ProvingOp for BtcBlockspaceHandler {
    type Prover = BtcBlockspaceProver;

    async fn create_dep_tasks(
        &self,
        _task_tracker: Arc<Mutex<TaskTracker>>,
        _proof_id: ProofId,
        _hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError> {
        Ok(vec![])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<BlockspaceProofInput, ProvingTaskError> {
        let height = match task_id.id() {
            ProofId::BtcBlockspace(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("BtcBlockspace".to_string())),
        };

        let block = self.btc_client.get_block_at(*height).await.unwrap();

        Ok(BlockspaceProofInput {
            rollup_params: get_pm_rollup_params(),
            block,
        })
    }
}
