use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmHost;
use tokio::sync::Mutex;

use super::{cl_stf::ClStfHandler, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, primitives::vms::ProofVm, task::TaskTracker};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClAggHandler {
    cl_stf_handler: Arc<ClStfHandler>,
}

impl ClAggHandler {
    /// Creates a new CL operations instance.
    pub fn new(cl_stf_handler: Arc<ClStfHandler>) -> Self {
        Self { cl_stf_handler }
    }
}

impl ProvingOp for ClAggHandler {
    type Prover = ClAggProver;

    async fn create_dep_tasks(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError> {
        let (start_height, end_height) = match proof_id {
            ProofId::ClAgg(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("ClAgg".to_string())),
        };

        let len = (end_height - start_height) as usize + 1;
        let mut deps = Vec::with_capacity(len);
        for height in start_height..=end_height {
            let proof_id = ProofId::ClStf(height);
            self.cl_stf_handler
                .create_task(task_tracker.clone(), proof_id, hosts)
                .await?;
            deps.push(proof_id);
        }

        Ok(deps)
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<ClAggInput, ProvingTaskError> {
        let (start_height, end_height) = match task_id.id() {
            ProofId::ClAgg(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("ClAgg".to_string())),
        };

        let mut batch = Vec::new();
        for height in *start_height..=*end_height {
            let id = ProofId::ClStf(height);
            let proof_key = ProofKey::new(id, *task_id.host());
            let proof = db
                .get_proof(proof_key)
                .map_err(ProvingTaskError::DatabaseError)?
                .ok_or(ProvingTaskError::ProofNotFound(proof_key))?;
            batch.push(proof);
        }

        let cl_stf_vk = hosts::get_host(ProofVm::CLProving).get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }
}
