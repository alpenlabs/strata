use std::sync::Arc;

use alloy_rpc_types::Block;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::{
    buf::Buf32,
    proof::{ProofContext, ProofKey, ProofZkVm},
};
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

    pub async fn get_block(&self, block_num: u64) -> Result<Block, ProvingTaskError> {
        self.el_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", block_num), false],
            )
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))
    }
}

impl ProvingOp for EvmEeHandler {
    type Prover = EvmEeProver;
    type Params = u64;

    async fn fetch_proof_contexts(
        &self,
        block_num: u64,
        _task_tracker: Arc<Mutex<TaskTracker>>,
        _db: &ProofDb,
        _hosts: &[ProofZkVm],
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError> {
        let block = self.get_block(block_num).await?;
        let blkid: Buf32 = block.header.hash.into();
        Ok((ProofContext::EvmEeStf(blkid), vec![]))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<ELProofInput, ProvingTaskError> {
        let block_hash = match task_id.context() {
            ProofContext::EvmEeStf(id) => id,
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };

        let witness: ELProofInput = self
            .el_client
            .request("strataee_getBlockWitness", rpc_params![block_hash, true])
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        Ok(witness)
    }
}
