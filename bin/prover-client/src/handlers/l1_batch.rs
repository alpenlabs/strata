use std::sync::Arc;

use bitcoin::params::MAINNET;
use strata_btcio::{reader::query::get_verification_state, rpc::BitcoinClient};
use strata_db::traits::{ProofDatabase, ProverDatabase};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::ZkVmHost;

use super::{btc::BtcBlockspaceHandler, ProvingOp};
use crate::{errors::ProvingTaskError, primitives::vms::ProofVm, hosts};

#[derive(Debug, Clone)]
pub struct L1BatchHandler {
    btc_dispatcher: Arc<BtcBlockspaceHandler>,
    btc_client: Arc<BitcoinClient>,
}

impl ProvingOp for L1BatchHandler {
    type Prover = L1BatchProver;

    async fn create_task(
        &self,
        task_tracker: &mut crate::task2::TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        let (start_height, end_height) = match task_id {
            ProofKey::L1Batch(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("L1Batch".to_string())),
        };

        let len = (end_height - start_height) as usize + 1;
        let mut deps = Vec::with_capacity(len);
        for height in *start_height..=*end_height {
            let proof_key = ProofKey::BtcBlockspace(height);
            self.btc_dispatcher
                .create_task(task_tracker, &proof_key)
                .await?;
            deps.push(proof_key);
        }

        task_tracker.insert_task(*task_id, deps)?;

        Ok(())
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProverDB,
    ) -> Result<L1BatchProofInput, ProvingTaskError> {
        let (start_height, end_height) = match task_id {
            ProofKey::L1Batch(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("L1Batch".to_string())),
        };

        let mut batch = Vec::new();
        for height in *start_height..=*end_height {
            let proof_key = ProofKey::BtcBlockspace(height);
            let proof = db
                .proof_db()
                .get_proof(proof_key)
                .map_err(ProvingTaskError::DatabaseError)?
                .ok_or(ProvingTaskError::ProofNotFound(proof_key))?;
            batch.push(proof);
        }

        let state = get_verification_state(
            self.btc_client.as_ref(),
            *start_height,
            &MAINNET.clone().into(),
        )
        .await
        .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let blockspace_vk = hosts::get_host(ProofVm::BtcProving).get_verification_key();

        Ok(L1BatchProofInput {
            batch,
            state,
            blockspace_vk,
        })
    }
}
