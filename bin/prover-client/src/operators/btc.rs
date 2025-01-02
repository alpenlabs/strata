use std::sync::Arc;

use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::{
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, prover::BtcBlockspaceProver};
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
    type Params = (u64, u64);

    async fn create_task(
        &self,
        block_range: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        _db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_height, end_height) = block_range;
        let start_block_id = self.get_id(start_height).await?;
        let end_block_id = self.get_id(end_height).await?;

        let context = ProofContext::BtcBlockspace(start_block_id, end_block_id);
        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(context, vec![])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<BlockScanProofInput, ProvingTaskError> {
        let _blkids = match task_id.context() {
            ProofContext::BtcBlockspace(start_block_id, end_block_id) => {
                (start_block_id, end_block_id)
            }
            _ => return Err(ProvingTaskError::InvalidInput("BtcBlockscan".to_string())),
        };

        // TODO: Fetch blocks from the BTC client.
        // let block = self.btc_client.get_block(&blkid.into()).await.unwrap();
        let blocks = vec![];

        Ok(BlockScanProofInput {
            rollup_params: self.rollup_params.as_ref().clone(),
            blocks,
        })
    }
}
