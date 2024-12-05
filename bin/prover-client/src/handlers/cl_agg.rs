use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_db::traits::{ProofDatabase, ProverDatabase};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::ZkVmHost;

use super::{cl_stf::ClStfHandler, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, primitives::vms::ProofVm, task2::TaskTracker};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClAggHandler {
    cl_client: HttpClient,
    cl_stf_handler: Arc<ClStfHandler>,
}

impl ClAggHandler {
    /// Creates a new CL operations instance.
    pub fn new(cl_client: HttpClient, cl_stf_handler: Arc<ClStfHandler>) -> Self {
        Self {
            cl_client,
            cl_stf_handler,
        }
    }
}

impl ProvingOp for ClAggHandler {
    type Prover = ClAggProver;

    async fn create_task(
        &self,
        task_tracker: &mut TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        let (start_height, end_height) = match task_id {
            ProofKey::ClAgg(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("ClAgg".to_string())),
        };

        let len = (end_height - start_height) as usize + 1;
        let mut deps = Vec::with_capacity(len);
        for height in *start_height..=*end_height {
            let proof_key = ProofKey::BtcBlockspace(height);
            self.cl_stf_handler
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
    ) -> Result<ClAggInput, ProvingTaskError> {
        let (start_height, end_height) = match task_id {
            ProofKey::ClAgg(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("ClAgg".to_string())),
        };

        let mut batch = Vec::new();
        for height in *start_height..=*end_height {
            let proof_key = ProofKey::ClStf(height);
            let proof = db
                .proof_db()
                .get_proof(proof_key)
                .map_err(ProvingTaskError::DatabaseError)?
                .ok_or(ProvingTaskError::ProofNotFound(proof_key))?;
            batch.push(proof);
        }

        let cl_stf_vk = hosts::get_host(ProofVm::CLProving).get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }
}
