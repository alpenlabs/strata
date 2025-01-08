use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey};
use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_state::id::L2BlockId;
use tokio::sync::Mutex;
use tracing::error;

use super::{cl_stf::ClStfOperator, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] for Consensus Layer (CL) Aggregated Proof.
///
/// It is responsible for managing the data and tasks required to generate proofs of CL Aggregation.
/// It fetches the necessary inputs for the [`ClAggProver`] by: utilizing the [`ClStfOperator`] to
/// create and manage proving tasks for CL STFs. The resulting CL STF proofs are incorporated as
/// part of the   input for the CL STF proof.
#[derive(Debug, Clone)]
pub struct ClAggOperator {
    cl_stf_operator: Arc<ClStfOperator>,
}

type BlockBatches = Vec<(L2BlockId, L2BlockId)>;

impl ClAggOperator {
    /// Creates a new CL operations instance.
    pub fn new(cl_stf_operator: Arc<ClStfOperator>) -> Self {
        Self { cl_stf_operator }
    }
}

impl ProvingOp for ClAggOperator {
    type Prover = ClAggProver;
    type Params = BlockBatches;

    fn construct_proof_ctx(
        &self,
        batches: &Self::Params,
    ) -> Result<ProofContext, ProvingTaskError> {
        // Extract first and last block IDs from batches, error if empty
        let (start_blkid, end_blkid) = match (batches.first(), batches.last()) {
            (Some(first), Some(last)) => (first.0, last.1),
            _ => {
                error!("Aggregation task with empty batch");
                return Err(ProvingTaskError::InvalidInput(
                    "Aggregation task with empty batch".into(),
                ));
            }
        };

        Ok(ProofContext::ClAgg(start_blkid, end_blkid))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<ClAggInput, ProvingTaskError> {
        let (start_blkid, end_blkid) = match task_id.context() {
            ProofContext::ClAgg(start, end) => (start, end),
            _ => return Err(ProvingTaskError::InvalidInput("ClAgg".to_string())),
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

        let cl_stf_vk = hosts::get_verification_key(&ProofKey::new(
            ProofContext::ClStf(*start_blkid, *end_blkid),
            *task_id.host(),
        ));
        Ok(ClAggInput { batch, cl_stf_vk })
    }

    async fn create_deps_tasks(
        &self,
        batches: BlockBatches,
        db: &ProofDb,
        task_tracker: Arc<Mutex<TaskTracker>>,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let mut cl_stf_deps = Vec::with_capacity(batches.len());

        for (start_blkid, end_blkid) in batches {
            let proof_keys = self
                .cl_stf_operator
                .create_task((start_blkid, end_blkid), task_tracker.clone(), db)
                .await?;
            cl_stf_deps.extend(proof_keys);
        }
        Ok(cl_stf_deps)
    }
}
