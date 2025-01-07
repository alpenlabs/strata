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
use strata_rpc_types::RpcBlockHeader;
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

    async fn get_l2_block_header(
        &self,
        blkid: L2BlockId,
    ) -> Result<RpcBlockHeader, ProvingTaskError> {
        let header = self
            .cl_client
            .get_header_by_id(blkid)
            .await
            .inspect_err(|_| error!(%blkid, "Failed to fetch corresponding ee data"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .ok_or_else(|| {
                error!(%blkid, "L2 Block not found");
                ProvingTaskError::InvalidWitness(format!("L2 Block {} not found", blkid))
            })?;

        Ok(header)
    }

    /// Retrieves the evm_ee block hash corresponding to the given L2 block ID
    pub async fn get_exec_id(&self, cl_block_id: L2BlockId) -> Result<Buf32, ProvingTaskError> {
        let header = self.get_l2_block_header(cl_block_id).await?;
        let block = self.evm_ee_operator.get_block(header.block_idx).await?;
        Ok(block.header.hash.into())
    }

    /// Retrieves the specified number of ancestor block IDs for the given block ID.
    pub async fn get_block_ancestors(
        &self,
        blkid: L2BlockId,
        n_ancestors: u64,
    ) -> Result<Vec<L2BlockId>, ProvingTaskError> {
        let mut ancestors = Vec::with_capacity(n_ancestors as usize);
        ancestors.push(blkid);

        let mut blkid = blkid;
        for _ in 0..n_ancestors {
            blkid = self.get_prev_block_id(blkid).await?;
            ancestors.push(blkid);
        }
        Ok(ancestors)
    }

    /// Retrieves the previous [`L2BlockId`] for the given `L2BlockId`
    pub async fn get_prev_block_id(&self, blkid: L2BlockId) -> Result<L2BlockId, ProvingTaskError> {
        let l2_block = self
            .cl_client
            .get_header_by_id(blkid)
            .await
            .inspect_err(|_| error!(%blkid, "Failed to fetch l2_header"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let prev_block: Buf32 = l2_block
            .ok_or_else(|| {
                error!(%blkid, "L2 Block not found");
                ProvingTaskError::InvalidWitness(format!("L2 Block {} not found", blkid))
            })?
            .prev_block
            .into();

        Ok(prev_block.into())
    }
}

impl ProvingOp for ClStfOperator {
    type Prover = ClStfProver;
    type Params = (L2BlockId, L2BlockId);

    async fn create_task(
        &self,
        block_range: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_block_id, end_block_id) = block_range;

        let el_start_block_id = self.get_exec_id(start_block_id).await?;
        let el_end_block_id = self.get_exec_id(end_block_id).await?;

        let evm_ee_tasks = self
            .evm_ee_operator
            .create_task(
                (el_start_block_id, el_end_block_id),
                task_tracker.clone(),
                db,
            )
            .await?;

        let evm_ee_id = evm_ee_tasks
            .first()
            .ok_or(ProvingTaskError::NoTasksFound)?
            .context();

        let cl_stf_id = ProofContext::ClStf(start_block_id, end_block_id);

        db.put_proof_deps(cl_stf_id, vec![*evm_ee_id])
            .map_err(ProvingTaskError::DatabaseError)?;

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(cl_stf_id, vec![*evm_ee_id], db)
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

        let start_block = self.get_l2_block_header(start_block_hash).await?;
        let end_block = self.get_l2_block_header(end_block_hash).await?;
        let num_blocks = end_block.block_idx - start_block.block_idx;

        // Get ancestor blocks and reverse to oldest-first order
        let mut l2_block_ids = self.get_block_ancestors(end_block_hash, num_blocks).await?;
        l2_block_ids.reverse();

        let mut stf_witness_payloads = Vec::new();
        for l2_block_id in l2_block_ids.clone() {
            let raw_witness: Vec<u8> = self
                .cl_client
                .get_cl_block_witness_raw(l2_block_id)
                .await
                .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
            stf_witness_payloads.push(raw_witness);
        }

        let evm_ee_ids = db
            .get_proof_deps(*task_id.context())
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::DependencyNotFound(*task_id))?;
        let evm_ee_id = evm_ee_ids.first().ok_or(ProvingTaskError::NoTasksFound)?;
        let evm_ee_key = ProofKey::new(*evm_ee_id, *task_id.host());
        let evm_ee_proof = db
            .get_proof(evm_ee_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(evm_ee_key))?;
        let evm_ee_vk = hosts::get_verification_key(&evm_ee_key);

        let rollup_params = self.rollup_params.as_ref().clone();
        Ok(ClStfInput {
            rollup_params,
            stf_witness_payloads,
            evm_ee_proof,
            evm_ee_vk,
        })
    }
}
