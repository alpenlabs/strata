use std::sync::Arc;

use bitcoin::params::MAINNET;
use strata_btcio::{
    reader::query::get_verification_state,
    rpc::{
        traits::{Reader, Wallet},
        BitcoinClient,
    },
};
use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey};
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_rocksdb::prover::db::ProofDb;
use tokio::sync::Mutex;
use tracing::error;

use super::{btc::BtcBlockspaceOperator, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, task::TaskTracker};

#[derive(Debug, Clone)]
pub struct L1BatchController {
    btc_client: Arc<BitcoinClient>,
    btc_blockspace_operator: Arc<BtcBlockspaceOperator>,
}

impl L1BatchController {
    pub fn new(
        btc_client: Arc<BitcoinClient>,
        btc_blockspace_operator: Arc<BtcBlockspaceOperator>,
    ) -> Self {
        Self {
            btc_client,
            btc_blockspace_operator,
        }
    }
}

impl ProvingOp for L1BatchController {
    type Prover = L1BatchProver;
    type Params = (u64, u64);

    async fn create_task(
        &self,
        params: (u64, u64),
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_height, end_height) = params;

        let len = (end_height - start_height) as usize + 1;
        let mut btc_deps = Vec::with_capacity(len);

        let start_blkid = self.btc_blockspace_operator.get_id(start_height).await?;
        let end_blkid = self.btc_blockspace_operator.get_id(end_height).await?;
        let l1_batch_proof_id = ProofContext::L1Batch(start_blkid, end_blkid);

        for height in start_height..=end_height {
            let blkid = self.btc_blockspace_operator.get_id(height).await?;
            let proof_id = ProofContext::BtcBlockspace(blkid);
            self.btc_blockspace_operator
                .create_task(height, task_tracker.clone(), db)
                .await?;
            btc_deps.push(proof_id);
        }

        db.put_proof_deps(l1_batch_proof_id, btc_deps.clone())
            .map_err(ProvingTaskError::DatabaseError)?;

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(l1_batch_proof_id, btc_deps)
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<L1BatchProofInput, ProvingTaskError> {
        let (start_blkid, _) = match task_id.context() {
            ProofContext::L1Batch(start, end) => (*start, end),
            _ => return Err(ProvingTaskError::InvalidInput("L1Batch".to_string())),
        };

        let deps = db
            .get_proof_deps(*task_id.context())
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::DependencyNotFound(*task_id))?;

        let mut batch = Vec::new();
        for proof_id in deps {
            let proof_key = ProofKey::new(proof_id, *task_id.host());
            let proof = db
                .get_proof(proof_key)
                .map_err(ProvingTaskError::DatabaseError)?
                .ok_or(ProvingTaskError::ProofNotFound(proof_key))?;
            batch.push(proof);
        }

        let start_block = self
            .btc_client
            .get_block(&start_blkid.into())
            .await
            .inspect_err(|_| error!(%start_blkid, "Failed to fetch BTC block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let start_height = self
            .btc_client
            .get_transaction(
                &start_block
                    .coinbase()
                    .expect("expect coinbase tx")
                    .compute_txid(),
            )
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .block_height();

        let state = get_verification_state(
            self.btc_client.as_ref(),
            start_height,
            &MAINNET.clone().into(),
        )
        .await
        .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let blockspace_vk = hosts::get_verification_key(&ProofKey::new(
            ProofContext::BtcBlockspace(start_blkid),
            *task_id.host(),
        ));

        Ok(L1BatchProofInput {
            batch,
            state,
            blockspace_vk,
        })
    }
}
