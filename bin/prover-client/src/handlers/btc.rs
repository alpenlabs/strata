use std::sync::Arc;

use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_btc_blockspace::{logic::BlockspaceProofInput, prover::BtcBlockspaceProver};
use strata_rocksdb::prover::db::ProverDB;

use super::ProofHandler;
use crate::{errors::ProvingTaskError, proving_ops::btc_ops::get_pm_rollup_params};

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

impl ProofHandler for BtcBlockspaceHandler {
    type Prover = BtcBlockspaceProver;

    async fn create_task(
        &self,
        task_tracker: &mut crate::task2::TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        task_tracker.insert_task(*task_id, vec![])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProverDB,
    ) -> Result<BlockspaceProofInput, ProvingTaskError> {
        let height = match task_id {
            ProofKey::BtcBlockspace(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("BtcBlockspace".to_string())),
        };

        let block = self.btc_client.get_block_at(*height).await.unwrap();

        Ok(BlockspaceProofInput {
            rollup_params: get_pm_rollup_params(),
            block,
        })
    }
}
