use std::sync::Arc;

use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::proof::{ProofContext, ProofKey, ProofZkVm};
use strata_proofimpl_btc_blockspace::{logic::BlockspaceProofInput, prover::BtcBlockspaceProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_state::l1::L1BlockId;
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

    pub async fn get_id(&self, block_num: u64) -> Result<L1BlockId, ProvingTaskError> {
        Ok(self
            .btc_client
            .get_block_hash(block_num)
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .into())
    }
}

impl ProvingOp for BtcBlockspaceHandler {
    type Prover = BtcBlockspaceProver;
    type Params = u64;

    async fn fetch_proof_ids(
        &self,
        block_num: u64,
        _task_tracker: Arc<Mutex<TaskTracker>>,
        _db: &ProofDb,
        _hosts: &[ProofZkVm],
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError> {
        Ok((
            ProofContext::BtcBlockspace(self.get_id(block_num).await?),
            vec![],
        ))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<BlockspaceProofInput, ProvingTaskError> {
        let blkid = match task_id.context() {
            ProofContext::BtcBlockspace(id) => *id,
            _ => return Err(ProvingTaskError::InvalidInput("BtcBlockspace".to_string())),
        };

        let block = self.btc_client.get_block(&blkid.into()).await.unwrap();

        Ok(BlockspaceProofInput {
            rollup_params: get_pm_rollup_params(),
            block,
        })
    }
}
