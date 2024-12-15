use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::ProofDatabase;
use strata_primitives::{
    buf::Buf32,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_cl_stf::prover::{ClStfInput, ClStfProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::RpcBlockHeader;
use strata_state::id::L2BlockId;
use tokio::sync::Mutex;

use super::{evm_ee::EvmEeHandler, utils::get_pm_rollup_params, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, task::TaskTracker};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClStfHandler {
    cl_client: HttpClient,
    evm_ee_handler: Arc<EvmEeHandler>,
}

impl ClStfHandler {
    /// Creates a new CL operations instance.
    pub fn new(cl_client: HttpClient, evm_ee_handler: Arc<EvmEeHandler>) -> Self {
        Self {
            cl_client,
            evm_ee_handler,
        }
    }

    pub async fn get_id(&self, block_num: u64) -> Result<L2BlockId, ProvingTaskError> {
        let l2_headers: Option<Vec<RpcBlockHeader>> = self
            .cl_client
            .request("strata_getHeadersAtIdx", rpc_params![block_num])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let cl_stf_id_buf: Buf32 = l2_headers
            .expect("invalid height")
            .first()
            .expect("at least one l2 blockid")
            .block_id
            .into();
        Ok(cl_stf_id_buf.into())
    }

    pub async fn get_slot(&self, id: L2BlockId) -> Result<u64, ProvingTaskError> {
        let header: RpcBlockHeader = self
            .cl_client
            .request("strata_getHeaderById", rpc_params![id])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        Ok(header.block_idx)
    }
}

impl ProvingOp for ClStfHandler {
    type Prover = ClStfProver;
    type Params = u64;

    async fn fetch_proof_contexts(
        &self,
        block_num: u64,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError> {
        let evm_ee_tasks = self
            .evm_ee_handler
            .create_task(block_num, task_tracker.clone(), db)
            .await?;
        let evm_ee_id = evm_ee_tasks
            .first()
            .expect("creation of task should result on at least one key")
            .context();

        let cl_stf_id = ProofContext::ClStf(self.get_id(block_num).await?);

        db.put_proof_deps(cl_stf_id, vec![*evm_ee_id])
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok((cl_stf_id, vec![*evm_ee_id]))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<ClStfInput, ProvingTaskError> {
        let block_id = match task_id.context() {
            ProofContext::ClStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };
        let block_num = self.get_slot(*block_id).await?;
        let raw_witness: Option<Vec<u8>> = self
            .cl_client
            .request("strata_getCLBlockWitness", rpc_params![block_num])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        let witness = raw_witness.ok_or(ProvingTaskError::WitnessNotFound)?;
        let (pre_state, l2_block) = borsh::from_slice(&witness)?;

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

        Ok(ClStfInput {
            rollup_params: get_pm_rollup_params(),
            pre_state,
            l2_block,
            evm_ee_proof,
            evm_ee_vk,
        })
    }
}
