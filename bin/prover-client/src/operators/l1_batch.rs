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
/// It is responsible for managing the data and tasks required to generate proofs for L1 Batch. It
/// fetches the necessary inputs for the [`L1BatchProver`] by:
///
/// - Utilizing the [`BtcBlockspaceOperator`] to create and manage proving tasks for BTC Blockspace.
///   The resulting BTC Blockspace proofs are incorporated as part of the input for the CL STF
///   proof.
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

    async fn get_block_at(&self, height: u64) -> Result<bitcoin::Block, ProvingTaskError> {
        self.btc_client
            .get_block_at(height)
            .await
            .inspect_err(|_| error!(%height, "Failed to fetch BTC block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))
    }

    async fn get_block(&self, block_id: L1BlockId) -> Result<bitcoin::Block, ProvingTaskError> {
        self.btc_client
            .get_block(&block_id.into())
            .await
            .inspect_err(|_| error!(%block_id, "Failed to fetch BTC block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))
    }

    async fn get_block_height(&self, block_id: L1BlockId) -> Result<u64, ProvingTaskError> {
        let block = self
            .btc_client
            .get_block(&block_id.into())
            .await
            .inspect_err(|_| error!(%block_id, "Failed to fetch BTC block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let block_height = self
            .btc_client
            .get_transaction(&block.coinbase().expect("expect coinbase tx").compute_txid())
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .block_height();

        Ok(block_height)
    }

    /// Retrieves the specified number of ancestor block IDs for the given block ID.
    pub async fn get_block_ancestors(
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
}

impl ProvingOp for L1BatchOperator {
    type Prover = L1BatchProver;
    type Params = (u64, u64);

    async fn create_task(
        &self,
        params: (u64, u64),
        task_tracker: Arc<Mutex<TaskTracker>>,
        _db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_height, end_height) = params;

        let start_blkid = self.get_block_at(start_height).await?.block_hash().into();
        let end_blkid = self.get_block_at(end_height).await?.block_hash().into();
        let l1_batch_proof_id = ProofContext::L1Batch(start_blkid, end_blkid);

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(l1_batch_proof_id, vec![])
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
