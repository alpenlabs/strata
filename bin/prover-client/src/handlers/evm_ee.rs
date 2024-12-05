use alloy_rpc_types::Block;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::proof::ProofKey;
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput};

use super::ProvingOp;
use crate::{errors::ProvingTaskError, task2::TaskTracker};

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

    async fn create_task(
        &self,
        task_tracker: &mut TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        task_tracker.insert_task(*task_id, vec![])
    }

    async fn fetch_input(
        &self,
        task_id: &strata_primitives::proof::ProofKey,
        _db: &strata_rocksdb::prover::db::ProverDB,
    ) -> Result<ELProofInput, ProvingTaskError> {
        let block_num = match task_id {
            ProofKey::EvmEeStf(id) => id,
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
