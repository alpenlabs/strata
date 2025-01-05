use std::sync::Arc;

use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::{
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_btc_blockspace::{logic::BlockspaceProofInput, prover::BtcBlockspaceProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_state::l1::L1BlockId;
use tokio::sync::Mutex;
use tracing::error;

use super::ProvingOp;
use crate::{errors::ProvingTaskError, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] trait for Bitcoin blockspace proof generation.
///
/// It interfaces with the Bitcoin blockchain via a [`BitcoinClient`] to fetch the necessary data
/// required by the [`BtcBlockspaceProver`] for the proof generation.
#[derive(Debug, Clone)]
pub struct BtcBlockspaceOperator {
    btc_client: Arc<BitcoinClient>,
    rollup_params: Arc<RollupParams>,
}

impl BtcBlockspaceOperator {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>, rollup_params: Arc<RollupParams>) -> Self {
        Self {
            btc_client,
            rollup_params,
        }
    }

    /// Retrieves the [`L1BlockId`] for a given block number.
    pub async fn get_id(&self, block_num: u64) -> Result<L1BlockId, ProvingTaskError> {
        Ok(self
            .btc_client
            .get_block_hash(block_num)
            .await
            .inspect_err(|_| error!(%block_num, "Failed to fetch BTC BlockId"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .into())
    }
}

impl ProvingOp for BtcBlockspaceOperator {
    type Prover = BtcBlockspaceProver;
    type Params = u64;

    async fn create_task(
        &self,
        block_num: u64,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let context = ProofContext::BtcBlockspace(self.get_id(block_num).await?);
        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(context, vec![], db)
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

        let block = self
            .btc_client
            .get_block(&blkid.into())
            .await
            .inspect_err(|_| error!(%blkid, "Failed to fetch BTC BlockId"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        Ok(BlockspaceProofInput {
            rollup_params: self.rollup_params.as_ref().clone(),
            block,
        })
    }
}
