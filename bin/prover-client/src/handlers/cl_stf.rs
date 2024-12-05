use std::sync::Arc;

use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_db::traits::{ProofDatabase, ProverDatabase};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_cl_stf::prover::{ClStfInput, ClStfProver};
use strata_zkvm::ZkVmHost;

use super::{evm_ee::EvmEeHandler, ProofHandler};
use crate::{
    errors::ProvingTaskError, primitives::vms::ProofVm, proving_ops::btc_ops::get_pm_rollup_params,
    zkvm,
};

/// Operations required for CL block proving tasks.
#[derive(Debug, Clone)]
pub struct ClStfHandler {
    cl_client: HttpClient,
    el_dispatcher: Arc<EvmEeHandler>,
}

impl ClStfHandler {
    /// Creates a new CL operations instance.
    pub fn new(cl_client: HttpClient, el_dispatcher: Arc<EvmEeHandler>) -> Self {
        Self {
            cl_client,
            el_dispatcher,
        }
    }
}

impl ProofHandler for ClStfHandler {
    type Prover = ClStfProver;

    async fn fetch_input(
        &self,
        task_id: &strata_primitives::proof::ProofKey,
        db: &strata_rocksdb::prover::db::ProverDB,
    ) -> Result<ClStfInput, ProvingTaskError> {
        let block_num = match task_id {
            ProofKey::ClStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };
        let raw_witness: Option<Vec<u8>> = self
            .cl_client
            .request("strata_getCLBlockWitness", rpc_params![block_num])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        let witness = raw_witness.ok_or(ProvingTaskError::WitnessNotFound)?;
        let (pre_state, l2_block) = borsh::from_slice(&witness)?;

        let evm_ee_key = ProofKey::EvmEeStf(*block_num);
        let evm_ee_proof = db
            .proof_db()
            .get_proof(evm_ee_key)
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::ProofNotFound(evm_ee_key))?;
        let evm_ee_vk = zkvm::get_host(ProofVm::ELProving).get_verification_key();

        Ok(ClStfInput {
            rollup_params: get_pm_rollup_params(),
            pre_state,
            l2_block,
            evm_ee_proof,
            evm_ee_vk,
        })
    }
}
