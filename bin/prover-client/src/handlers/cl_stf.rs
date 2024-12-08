use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_cl_stf::prover::{ClStfInput, ClStfProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmHost;
use tokio::sync::Mutex;

use super::{evm_ee::EvmEeHandler, utils::get_pm_rollup_params, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, primitives::vms::ProofVm, task::TaskTracker};

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
}

impl ProvingOp for ClStfHandler {
    type Prover = ClStfProver;

    async fn create_dep_tasks(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError> {
        let block_num = match proof_id {
            ProofId::ClStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("ClStf".to_string())),
        };

        let ee_task = ProofId::EvmEeStf(block_num);
        self.evm_ee_handler
            .create_task(task_tracker.clone(), ee_task, hosts)
            .await?;

        Ok(vec![ee_task])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<ClStfInput, ProvingTaskError> {
        let block_num = match task_id.id() {
            ProofId::ClStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };
        let raw_witness: Option<Vec<u8>> = self
            .cl_client
            .request("strata_getCLBlockWitness", rpc_params![block_num])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        let witness = raw_witness.ok_or(ProvingTaskError::WitnessNotFound)?;
        let (pre_state, l2_block) = borsh::from_slice(&witness)?;

        let id = ProofId::EvmEeStf(*block_num);
        let evm_ee_key = ProofKey::new(id, *task_id.host());
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
