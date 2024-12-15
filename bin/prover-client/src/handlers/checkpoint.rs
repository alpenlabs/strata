use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey};
use strata_proofimpl_checkpoint::prover::{CheckpointProver, CheckpointProverInput};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::RpcCheckpointInfo;
use strata_zkvm::AggregationInput;
use tokio::sync::Mutex;

use super::{
    cl_agg::ClAggHandler, l1_batch::L1BatchHandler, utils::get_pm_rollup_params, ProvingOp,
};
use crate::{errors::ProvingTaskError, hosts, task::TaskTracker};

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
    type Params = u64;

    async fn fetch_proof_contexts(
        &self,
        ckp_idx: u64,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError> {
        let checkpoint_info = self.fetch_info(ckp_idx).await?;

        let ckp_proof_id = ProofContext::Checkpoint(ckp_idx);

        let l1_batch_keys = self
            .l1_batch_dispatcher
            .create_task(checkpoint_info.l1_range, task_tracker.clone(), db)
            .await?;
        let l1_batch_id = l1_batch_keys.first().expect("at least one").context();

        let l2_batch_keys = self
            .l2_batch_dispatcher
            .create_task(checkpoint_info.l2_range, task_tracker.clone(), db)
            .await?;
        let l2_batch_id = l2_batch_keys.first().expect("at least one").context();

        let deps = vec![*l1_batch_id, *l2_batch_id];

        db.put_proof_deps(ckp_proof_id, deps.clone())
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok((ckp_proof_id, deps))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<CheckpointProverInput, ProvingTaskError> {
        let deps = db
            .get_proof_deps(*task_id.context())
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::DependencyNotFound(*task_id))?;

        let l1_batch_id = deps[0];
        let l1_batch_key = ProofKey::new(l1_batch_id, *task_id.host());
        let l1_batch_proof = db
            .get_proof(l1_batch_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(l1_batch_key))?;
        let l1_batch_vk = hosts::get_verification_key(&l1_batch_key);
        let l1_batch = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let cl_agg_id = deps[1];
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
