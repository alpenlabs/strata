use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_db::traits::ProofDatabase;
use strata_primitives::{
    buf::Buf32,
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_checkpoint::prover::{CheckpointProver, CheckpointProverInput};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_api::StrataApiClient;
use strata_rpc_types::RpcCheckpointInfo;
use strata_state::id::L2BlockId;
use strata_zkvm::AggregationInput;
use tokio::sync::Mutex;
use tracing::{error, info};

use super::{cl_agg::ClAggOperator, l1_batch::L1BatchOperator, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] for Checkpoint Proof.
///
/// It is responsible for managing the data and tasks required to generate Checkpoint Proof. It
/// fetches the necessary inputs for the [`CheckpointProver`] by:
///
/// - utilizing the [`L1BatchOperator`] to create and manage proving tasks for L1Batch. The
///   resulting L1 Batch proof is incorporated as part of the input for the Checkpoint Proof.
/// - utilizing the [`ClAggOperator`] to create and manage proving tasks for CL Aggregation. The
///   resulting CL Aggregated proof is incorporated as part of the input for the Checkpoint Proof.
#[derive(Debug, Clone)]
pub struct CheckpointOperator {
    cl_client: HttpClient,
    l1_batch_operator: Arc<L1BatchOperator>,
    l2_batch_operator: Arc<ClAggOperator>,
    rollup_params: Arc<RollupParams>,
}

impl CheckpointOperator {
    /// Creates a new BTC operations instance.
    pub fn new(
        cl_client: HttpClient,
        l1_batch_operator: Arc<L1BatchOperator>,
        l2_batch_operator: Arc<ClAggOperator>,
        rollup_params: Arc<RollupParams>,
    ) -> Self {
        Self {
            cl_client,
            l1_batch_operator,
            l2_batch_operator,
            rollup_params,
        }
    }

    async fn fetch_ckp_info(&self, ckp_idx: u64) -> Result<RpcCheckpointInfo, ProvingTaskError> {
        self.cl_client
            .get_checkpoint_info(ckp_idx)
            .await
            .inspect_err(|_| error!(%ckp_idx, "Failed to fetch CheckpointInfo"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .ok_or(ProvingTaskError::WitnessNotFound)
    }

    /// Retrieves the [`L2BlockId`] for the given `block_num`
    pub async fn get_l2id(&self, block_num: u64) -> Result<L2BlockId, ProvingTaskError> {
        let l2_headers = self
            .cl_client
            .get_headers_at_idx(block_num)
            .await
            .inspect_err(|_| error!(%block_num, "Failed to fetch l2_headers"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let headers = l2_headers.ok_or_else(|| {
            error!(%block_num, "Failed to fetch L2 block");
            ProvingTaskError::InvalidWitness(format!("Invalid L2 block height {}", block_num))
        })?;

        let first_header: Buf32 = headers
            .first()
            .ok_or_else(|| {
                ProvingTaskError::InvalidWitness(format!("Invalid L2 block height {}", block_num))
            })?
            .block_id
            .into();

        Ok(first_header.into())
    }

    /// Retrieves the latest checkpoint index
    pub async fn fetch_latest_ckp_idx(&self) -> Result<u64, ProvingTaskError> {
        self.cl_client
            .get_latest_checkpoint_index(None)
            .await
            .inspect_err(|_| error!("Failed to fetch latest checkpoint"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .ok_or(ProvingTaskError::WitnessNotFound)
    }
}

impl ProvingOp for CheckpointOperator {
    type Prover = CheckpointProver;
    type Params = u64;

    async fn create_task(
        &self,
        ckp_idx: u64,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        info!(%ckp_idx, "Start creating task");
        let checkpoint_info = self.fetch_ckp_info(ckp_idx).await?;

        let ckp_proof_id = ProofContext::Checkpoint(ckp_idx);

        // Doing the manual block idx to id transformation. Will be removed once checkpoint_info
        // include the range in terms of block_id.
        // https://alpenlabs.atlassian.net/browse/STR-756
        let start_l1_block_id = self
            .l1_batch_operator
            .get_block_at(checkpoint_info.l1_range.0)
            .await?;
        let end_l1_block_id = self
            .l1_batch_operator
            .get_block_at(checkpoint_info.l1_range.1)
            .await?;

        let l1_batch_keys = self
            .l1_batch_operator
            .create_task(
                (start_l1_block_id, end_l1_block_id),
                task_tracker.clone(),
                db,
            )
            .await?;
        let l1_batch_id = l1_batch_keys
            .first()
            .ok_or(ProvingTaskError::NoTasksFound)?
            .context();

        // Doing the manual block idx to id transformation. Will be removed once checkpoint_info
        // include the range in terms of block_id.
        // https://alpenlabs.atlassian.net/browse/STR-756
        let start_l2_idx = self.get_l2id(checkpoint_info.l2_range.0).await?;
        let end_l2_idx = self.get_l2id(checkpoint_info.l2_range.1).await?;
        let l2_range = vec![(start_l2_idx, end_l2_idx)];
        info!(%ckp_idx, "Created tasks for L1 Batch");

        let l2_batch_keys = self
            .l2_batch_operator
            .create_task(l2_range, task_tracker.clone(), db)
            .await?;

        let l2_batch_id = l2_batch_keys
            .first()
            .ok_or(ProvingTaskError::NoTasksFound)?
            .context();
        info!(%ckp_idx, "Created tasks for L2 Batch");

        let deps = vec![*l1_batch_id, *l2_batch_id];

        db.put_proof_deps(ckp_proof_id, deps.clone())
            .map_err(ProvingTaskError::DatabaseError)?;

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(ckp_proof_id, deps, db)
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

        let rollup_params = self.rollup_params.as_ref().clone();
        Ok(CheckpointProverInput {
            rollup_params,
            l1_batch,
            l2_batch,
        })
    }
}
