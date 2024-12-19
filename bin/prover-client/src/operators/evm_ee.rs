use std::sync::Arc;

use alloy_rpc_types::Block;
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::{
    buf::Buf32,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput};
use strata_rocksdb::prover::db::ProofDb;
use tokio::sync::Mutex;
use tracing::error;

use super::ProvingOp;
use crate::{errors::ProvingTaskError, task_tracker::TaskTracker};

/// A struct that implements the [`ProvingOp`] trait for EVM Execution Environment (EE) State
/// Transition Function (STF) proofs.
///
/// It is responsible for interfacing with the `Reth` client and fetching necessary data required by
/// the [`EvmEeProver`] for the proof generation.
#[derive(Debug, Clone)]
pub struct EvmEeOperator {
    el_client: HttpClient,
}

impl EvmEeOperator {
    /// Creates a new EL operations instance.
    pub fn new(el_client: HttpClient) -> Self {
        Self { el_client }
    }

    /// Retrieves the EVM EE [`Block`] for a given block number.
    pub async fn get_block(&self, block_num: u64) -> Result<Block, ProvingTaskError> {
        self.el_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", block_num), false],
            )
            .await
            .inspect_err(|_| error!(%block_num, "Failed to fetch EVM Block"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))
    }
}

impl ProvingOp for EvmEeOperator {
    type Prover = EvmEeProver;
    type Params = u64;

    async fn create_task(
        &self,
        block_num: u64,
        task_tracker: Arc<Mutex<TaskTracker>>,
        _db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let block = self.get_block(block_num).await?;
        let blkid: Buf32 = block.header.hash.into();
        let context = ProofContext::EvmEeStf(blkid);

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(context, vec![])
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
