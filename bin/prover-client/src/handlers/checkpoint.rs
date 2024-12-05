use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::{ProofDatabase, ProverDatabase};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_checkpoint::prover::{CheckpointProver, CheckpointProverInput};
use strata_rocksdb::prover::db::ProverDB;
use strata_rpc_types::RpcCheckpointInfo;
use strata_zkvm::{AggregationInput, ZkVmHost};

use super::{
    cl_agg::{self, ClAggHandler},
    l1_batch::L1BatchHandler,
    ProofHandler,
};
use crate::{
    errors::ProvingTaskError, primitives::vms::ProofVm, proving_ops::btc_ops::get_pm_rollup_params,
    zkvm,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct CheckpointHandler {
    cl_client: HttpClient,
    l1_batch_dispatcher: Arc<L1BatchHandler>,
    l2_batch_dispatcher: Arc<ClAggHandler>,
}

impl CheckpointHandler {
    /// Creates a new BTC operations instance.
    pub fn new(
        cl_client: HttpClient,
        l1_batch_dispatcher: Arc<L1BatchHandler>,
        l2_batch_dispatcher: Arc<ClAggHandler>,
    ) -> Self {
        Self {
            cl_client,
            l1_batch_dispatcher,
            l2_batch_dispatcher,
        }
    }

    async fn fetch_info(&self, ckp_idx: u64) -> Result<RpcCheckpointInfo, ProvingTaskError> {
        Ok(self
            .cl_client
            .request::<Option<RpcCheckpointInfo>, _>(
                "strata_getCheckpointInfo",
                rpc_params![ckp_idx],
            )
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .ok_or(ProvingTaskError::WitnessNotFound)?)
    }
}

impl ProofHandler for CheckpointHandler {
    type Prover = CheckpointProver;

    async fn create_task(
        &self,
        task_tracker: &mut crate::task2::TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        let ckp_idx = match task_id {
            ProofKey::Checkpoint(idx) => idx,
            _ => return Err(ProvingTaskError::InvalidInput("Checkpoint".to_string())),
        };

        let checkpoint_info = self.fetch_info(*ckp_idx).await?;

        let l1_batch_key =
            ProofKey::L1Batch(checkpoint_info.l1_range.0, checkpoint_info.l1_range.1);
        self.l1_batch_dispatcher
            .create_task(task_tracker, &l1_batch_key)
            .await?;

        let cl_agg_key = ProofKey::ClAgg(checkpoint_info.l2_range.0, checkpoint_info.l2_range.1);
        self.l2_batch_dispatcher
            .create_task(task_tracker, &cl_agg_key)
            .await?;

        task_tracker.insert_task(*task_id, vec![l1_batch_key, cl_agg_key])?;

        Ok(())
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProverDB,
    ) -> Result<CheckpointProverInput, ProvingTaskError> {
        let ckp_idx = match task_id {
            ProofKey::Checkpoint(idx) => idx,
            _ => return Err(ProvingTaskError::InvalidInput("Checkpoint".to_string())),
        };

        let checkpoint_info = self.fetch_info(*ckp_idx).await?;

        let l1_batch_key =
            ProofKey::L1Batch(checkpoint_info.l1_range.0, checkpoint_info.l1_range.1);
        let l1_batch_proof = db
            .proof_db()
            .get_proof(l1_batch_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(l1_batch_key))?;
        let l1_batch_vk = zkvm::get_host(ProofVm::L1Batch).get_verification_key();
        let l1_batch = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let cl_agg_key = ProofKey::ClAgg(checkpoint_info.l2_range.0, checkpoint_info.l2_range.1);
        let cl_agg_proof = db
            .proof_db()
            .get_proof(cl_agg_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(cl_agg_key))?;
        let cl_agg_vk = zkvm::get_host(ProofVm::CLAggregation).get_verification_key();
        let l2_batch = AggregationInput::new(cl_agg_proof, cl_agg_vk);

        Ok(CheckpointProverInput {
            rollup_params: get_pm_rollup_params(),
            l1_batch,
            l2_batch,
        })
    }
}
