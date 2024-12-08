use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_checkpoint::prover::{CheckpointProver, CheckpointProverInput};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::RpcCheckpointInfo;
use strata_zkvm::{AggregationInput, ZkVmHost};
use tokio::sync::Mutex;

use super::{
    cl_agg::ClAggHandler, l1_batch::L1BatchHandler, utils::get_pm_rollup_params, ProvingOp,
};
use crate::{errors::ProvingTaskError, hosts, primitives::vms::ProofVm, task::TaskTracker};

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
        self.cl_client
            .request::<Option<RpcCheckpointInfo>, _>(
                "strata_getCheckpointInfo",
                rpc_params![ckp_idx],
            )
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .ok_or(ProvingTaskError::WitnessNotFound)
    }
}

impl ProvingOp for CheckpointHandler {
    type Prover = CheckpointProver;

    async fn create_dep_tasks(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError> {
        let ckp_idx = match proof_id {
            ProofId::Checkpoint(idx) => idx,
            _ => return Err(ProvingTaskError::InvalidInput("Checkpoint".to_string())),
        };

        let checkpoint_info = self.fetch_info(ckp_idx).await?;

        let l1_batch_id = ProofId::L1Batch(checkpoint_info.l1_range.0, checkpoint_info.l1_range.1);
        self.l1_batch_dispatcher
            .create_task(task_tracker.clone(), l1_batch_id, hosts)
            .await?;

        let l2_batch_id = ProofId::ClAgg(checkpoint_info.l2_range.0, checkpoint_info.l2_range.1);
        self.l2_batch_dispatcher
            .create_task(task_tracker.clone(), l2_batch_id, hosts)
            .await?;

        Ok(vec![l1_batch_id, l2_batch_id])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<CheckpointProverInput, ProvingTaskError> {
        let ckp_idx = match task_id.id() {
            ProofId::Checkpoint(idx) => idx,
            _ => return Err(ProvingTaskError::InvalidInput("Checkpoint".to_string())),
        };

        let checkpoint_info = self.fetch_info(*ckp_idx).await?;

        let l1_batch_id = ProofId::L1Batch(checkpoint_info.l1_range.0, checkpoint_info.l1_range.1);
        let l1_batch_key = ProofKey::new(l1_batch_id, *task_id.host());
        let l1_batch_proof = db
            .get_proof(l1_batch_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(l1_batch_key))?;
        let l1_batch_vk = hosts::get_verification_key(&l1_batch_key);
        let l1_batch = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let cl_agg_id = ProofId::ClAgg(checkpoint_info.l2_range.0, checkpoint_info.l2_range.1);
        let cl_agg_key = ProofKey::new(cl_agg_id, *task_id.host());
        let cl_agg_proof = db
            .get_proof(cl_agg_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(cl_agg_key))?;
        let cl_agg_vk = hosts::get_verification_key(&cl_agg_key);
        let l2_batch = AggregationInput::new(cl_agg_proof, cl_agg_vk);

        Ok(CheckpointProverInput {
            rollup_params: get_pm_rollup_params(),
            l1_batch,
            l2_batch,
        })
    }
}
