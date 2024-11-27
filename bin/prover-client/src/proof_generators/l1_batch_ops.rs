use std::sync::Arc;

use anyhow::anyhow;
use bitcoin::params::MAINNET;
use strata_btcio::{reader::query::get_verification_state, rpc::BitcoinClient};
use strata_db::traits::{ProverDataProvider, ProverDataStore, ProverDatabase};
use strata_primitives::vk::StrataProofId;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::VerificationKey;
use uuid::Uuid;

use super::{btc_ops::BtcBlockspaceProofGenerator, ProofGenerator};
use crate::{
    errors::ProvingTaskError,
    task2::{ProvingTask2, TaskTracker2},
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L1BatchProofGenerator {
    btc_dispatcher: Arc<BtcBlockspaceProofGenerator>,
    btc_client: Arc<BitcoinClient>,
}

impl L1BatchProofGenerator {
    /// Creates a new BTC operations instance.
    pub fn new(
        btc_dispatcher: Arc<BtcBlockspaceProofGenerator>,
        btc_client: Arc<BitcoinClient>,
    ) -> Self {
        Self {
            btc_dispatcher,
            btc_client,
        }
    }
}

impl ProofGenerator for L1BatchProofGenerator {
    // Range of l1 blocks
    type Prover = L1BatchProver;

    async fn create_task(
        &self,
        id: &StrataProofId,
        db: &ProverDB,
        task_tracker: Arc<TaskTracker2>,
    ) -> Result<Uuid, ProvingTaskError> {
        let mut dependencies = vec![];

        // Create btc tasks for each block in the range
        let (start, end) = match *id {
            StrataProofId::L1Batch(start, end) => (start, end),
            _ => {
                return Err(ProvingTaskError::InvalidInput(
                    "expected type L1Batch".to_string(),
                ))
            }
        };

        for btc_block_idx in start..=end {
            let btc_proof_id = StrataProofId::BtcBlockspace(btc_block_idx);
            let btc_task_id = self
                .btc_dispatcher
                .create_task(&btc_proof_id, db, task_tracker.clone())
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(btc_task_id);
        }

        let task = ProvingTask2::new(*id, dependencies.clone());
        let task_id = task_tracker.insert_task(task).await;
        db.prover_store().insert_task(task_id, *id);
        db.prover_store().insert_dependencies(task_id, dependencies);
        Ok(task_id)
    }

    async fn fetch_input(
        &self,
        id: &StrataProofId,
        db: &ProverDB,
    ) -> Result<<Self::Prover as strata_zkvm::ZkVmProver>::Input, anyhow::Error> {
        // Create btc tasks for each block in the range
        let (start_height, end_height) = match *id {
            StrataProofId::L1Batch(start, end) => (start, end),
            _ => return Err(anyhow!("invalid input")),
        };
        let state = get_verification_state(
            self.btc_client.as_ref(),
            start_height,
            &MAINNET.clone().into(),
        )
        .await?;

        let mut batch = vec![];
        for block_idx in start_height..=end_height {
            let btc_proof_id = StrataProofId::BtcBlockspace(block_idx);
            let proof = db
                .prover_provider()
                .get_proof(btc_proof_id)?
                .unwrap()
                .proof()
                .clone();
            batch.push(proof);
        }

        // TODO: fix this
        let blockspace_vk = VerificationKey::new(vec![]);
        Ok(L1BatchProofInput {
            batch,
            state,
            blockspace_vk,
        })
    }
}
