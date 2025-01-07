use std::sync::Arc;

use bitcoin::{params::MAINNET, Block};
use strata_btcio::{
    reader::query::get_verification_state,
    rpc::{
        traits::{Reader, Wallet},
        BitcoinClient,
    },
};
use strata_primitives::{
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_state::l1::L1BlockId;
use tokio::sync::Mutex;
use tracing::error;

use super::ProvingOp;
use crate::{errors::ProvingTaskError, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] trait for L1 Batch Proof generation.
///
/// It is responsible for managing the data to generate proofs for L1 Batch. It
/// fetches the necessary inputs for the [`L1BatchProver`] by:
/// - Fetching the Bitcoin blocks and verification state for the given block range.
/// - Interfacing with the Bitcoin Client to fetch additional required information for batch proofs.
#[derive(Debug, Clone)]
pub struct L1BatchOperator {
    btc_client: Arc<BitcoinClient>,
    rollup_params: Arc<RollupParams>,
}

impl L1BatchOperator {
    pub fn new(btc_client: Arc<BitcoinClient>, rollup_params: Arc<RollupParams>) -> Self {
        Self {
            btc_client,
            rollup_params,
        }
    }

    async fn get_block(&self, block_id: L1BlockId) -> Result<bitcoin::Block, ProvingTaskError> {
        self.btc_client
            .get_block(&block_id.into())
            .await
            .inspect_err(|_| error!(%block_id, "Failed to fetch BTC block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))
    }

    async fn get_block_height(&self, block_id: L1BlockId) -> Result<u64, ProvingTaskError> {
        let block = self.get_block(block_id).await?;

        let block_height = self
            .btc_client
            .get_transaction(&block.coinbase().expect("expect coinbase tx").compute_txid())
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .block_height();

        Ok(block_height)
    }

    /// Retrieves the specified number of ancestor block IDs for the given block ID.
    async fn get_block_ancestors(
        &self,
        block_id: L1BlockId,
        n_ancestors: u64,
    ) -> Result<Vec<Block>, ProvingTaskError> {
        let mut ancestors = Vec::with_capacity(n_ancestors as usize);
        let mut block_id = block_id;
        for _ in 0..=n_ancestors {
            let block = self.get_block(block_id).await?;
            block_id = block.header.prev_blockhash.into();
            ancestors.push(block);
        }

        Ok(ancestors)
    }

    /// Retrieves the block ID at the specified height.
    ///
    /// Note: This function will be removed once checkpoint_info includes the block ID range.
    /// Currently, it requires a manual L1 block index-to-ID conversion by the checkpoint operator.
    // https://alpenlabs.atlassian.net/browse/STR-756
    pub async fn get_block_at(&self, height: u64) -> Result<L1BlockId, ProvingTaskError> {
        let block_hash = self
            .btc_client
            .get_block_hash(height)
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        Ok(block_hash.into())
    }
}

impl ProvingOp for L1BatchOperator {
    type Prover = L1BatchProver;
    type Params = (L1BlockId, L1BlockId);

    async fn create_task(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_blkid, end_blkid) = params;
        let l1_batch_proof_id = ProofContext::L1Batch(start_blkid, end_blkid);

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(l1_batch_proof_id, vec![], db)
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<L1BatchProofInput, ProvingTaskError> {
        let (start_block_id, end_block_id) = match task_id.context() {
            ProofContext::L1Batch(start, end) => (*start, *end),
            _ => return Err(ProvingTaskError::InvalidInput("L1Batch".to_string())),
        };

        let start_height = self.get_block_height(start_block_id).await?;
        let end_height = self.get_block_height(end_block_id).await?;
        let num_blocks = end_height - start_height;

        // Get ancestor blocks and reverse to oldest-first order
        let mut blocks = self.get_block_ancestors(end_block_id, num_blocks).await?;
        blocks.reverse();

        let state = get_verification_state(
            self.btc_client.as_ref(),
            start_height,
            &MAINNET.clone().into(),
        )
        .await
        .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        Ok(L1BatchProofInput {
            blocks,
            state,
            rollup_params: self.rollup_params.as_ref().clone(),
        })
    }
}
