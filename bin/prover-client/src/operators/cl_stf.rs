use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_db::traits::ProofDatabase;
use strata_primitives::{
    buf::Buf32,
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_cl_stf::prover::{ClStfInput, ClStfProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_api::StrataApiClient;
use strata_state::id::L2BlockId;
use tokio::sync::Mutex;
use tracing::error;

use super::{evm_ee::EvmEeOperator, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] trait for Consensus Layer (CL) State Transition
/// Function (STF) proof generation.
///
/// It is responsible for managing the data and tasks required to generate proofs for CL state
/// transitions. It fetches the necessary inputs for the [`ClStfProver`] by:
///
/// - Utilizing the [`EvmEeOperator`] to create and manage proving tasks for EVM Execution
///   Environment (EE) STF proofs. The resulting EVM EE STF proof is incorporated as part of the
///   input for the CL STF proof.
/// - Interfacing with the CL Client to fetch additional required information for CL state
///   transition proofs.
#[derive(Debug, Clone)]
pub struct ClStfOperator {
    cl_client: HttpClient,
    evm_ee_operator: Arc<EvmEeOperator>,
    rollup_params: Arc<RollupParams>,
}

impl ClStfOperator {
    /// Creates a new CL operations instance.
    pub fn new(
        cl_client: HttpClient,
        evm_ee_operator: Arc<EvmEeOperator>,
        rollup_params: Arc<RollupParams>,
    ) -> Self {
        Self {
            cl_client,
            evm_ee_operator,
            rollup_params,
        }
    }

    /// Retrieves the [`L2BlockId`] for the given `block_num`
    pub async fn get_id(&self, block_num: u64) -> Result<L2BlockId, ProvingTaskError> {
        let l2_headers = self
            .cl_client
            .get_headers_at_idx(block_num)
            .await
            .inspect_err(|_| error!(%block_num, "Failed to fetch l2_headers"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let cl_stf_id_buf: Buf32 = l2_headers
            .expect("invalid height")
            .first()
            .expect("at least one l2 blockid")
            .block_id
            .into();
        Ok(cl_stf_id_buf.into())
    }

    /// Retrieves the previous [`L2Block`] for the given `block_id`
    pub async fn get_prev_block_id(
        &self,
        block_id: L2BlockId,
    ) -> Result<L2BlockId, ProvingTaskError> {
        let l2_block = self
            .cl_client
            .get_header_by_id(block_id)
            .await
            .inspect_err(|_| error!(%block_id, "Failed to fetch l2_header"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let prev_block: Buf32 = l2_block.expect("invalid height").block_id.into();

        Ok(prev_block.into())
    }

    /// Retrieves the slot num of the given [`L2BlockId`]
    pub async fn get_slot(&self, id: L2BlockId) -> Result<u64, ProvingTaskError> {
        let header = self
            .cl_client
            .get_header_by_id(id)
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .expect("invalid blkid");
        Ok(header.block_idx)
    }
}

impl ProvingOp for ClStfOperator {
    type Prover = ClStfProver;
    type Params = (u64, u64);

    async fn create_task(
        &self,
        block_range: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let evm_ee_tasks = self
            .evm_ee_operator
            .create_task(block_range, task_tracker.clone(), db)
            .await?;
        let evm_ee_id = evm_ee_tasks
            .first()
            .expect("creation of task should result on at least one key")
            .context();

        let (start_block_num, end_block_num) = block_range;
        let start_block_id = self.get_id(start_block_num).await?;
        let end_block_id = self.get_id(end_block_num).await?;
        let cl_stf_id = ProofContext::ClStf(start_block_id, end_block_id);

        db.put_proof_deps(cl_stf_id, vec![*evm_ee_id])
            .map_err(ProvingTaskError::DatabaseError)?;

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(cl_stf_id, vec![*evm_ee_id])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<ClStfInput, ProvingTaskError> {
        let (start_block_hash, end_block_hash) = match task_id.context() {
            ProofContext::ClStf(start, end) => (*start, *end),
            _ => return Err(ProvingTaskError::InvalidInput("CL_STF".to_string())),
        };

        let mut stf_witness_payloads = Vec::new();
        let mut blkid = end_block_hash;
        loop {
            let raw_witness: Option<Vec<u8>> = self
                .cl_client
                .get_cl_block_witness_raw(blkid)
                .await
                .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
            let witness = raw_witness.ok_or(ProvingTaskError::WitnessNotFound)?;
            stf_witness_payloads.push(witness);

            if blkid == start_block_hash {
                break;
            } else {
                blkid = self.get_prev_block_id(blkid).await?;
            }
        }

        let evm_ee_ids = db
            .get_proof_deps(*task_id.context())
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::DependencyNotFound(*task_id))?;
        let evm_ee_id = evm_ee_ids
            .first()
            .expect("should have at least a dependency");
        let evm_ee_key = ProofKey::new(*evm_ee_id, *task_id.host());
        let evm_ee_proof = db
            .get_proof(evm_ee_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(evm_ee_key))?;
        let evm_ee_vk = hosts::get_verification_key(&evm_ee_key);

        let rollup_params = self.rollup_params.as_ref().clone();
        Ok(ClStfInput {
            rollup_params,
            // pre_state,
            // l2_block,
            stf_witness_payloads,
            evm_ee_proof,
            evm_ee_vk,
        })
    }
}
