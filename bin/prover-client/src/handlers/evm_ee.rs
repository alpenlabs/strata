use std::sync::Arc;

use alloy_rpc_types::Block;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput};
use strata_rocksdb::prover::db::ProofDb;
use tokio::sync::Mutex;

use super::ProvingOp;
use crate::{errors::ProvingTaskError, task::TaskTracker};

/// Operations required for EL block proving tasks.
#[derive(Debug, Clone)]
pub struct EvmEeHandler {
    el_client: HttpClient,
}

impl EvmEeHandler {
    /// Creates a new EL operations instance.
    pub fn new(el_client: HttpClient) -> Self {
        Self { el_client }
    }
}

impl ProvingOp for EvmEeHandler {
    type Prover = EvmEeProver;

    async fn create_dep_tasks(
        &self,
        _task_tracker: Arc<Mutex<TaskTracker>>,
        _proof_id: ProofId,
        _hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError> {
        Ok(vec![])
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<ELProofInput, ProvingTaskError> {
        let block_num = match task_id.id() {
            ProofId::EvmEeStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };

        let block: Block = self
            .el_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", block_num), false],
            )
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        let block_hash = block.header.hash;
        let witness: ELProofInput = self
            .el_client
            .request("strataee_getBlockWitness", rpc_params![block_hash, true])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        Ok(witness)
    }
}
