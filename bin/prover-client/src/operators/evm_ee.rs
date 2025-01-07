use std::sync::Arc;

use alloy_rpc_types::{Block, Header};
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use strata_primitives::{
    buf::Buf32,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_evm_ee_stf::{
    primitives::EvmEeProofInput, prover::EvmEeProver, EvmBlockStfInput,
};
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

    /// Retrieves the EVM EE [`Block`] for a given block number.
    async fn get_block_header(&self, blkid: Buf32) -> Result<Header, ProvingTaskError> {
        let block: Block = self
            .el_client
            .request("eth_getBlockByHash", rpc_params![blkid, false])
            .await
            .inspect_err(|_| error!(%blkid, "Failed to fetch EVM Block Header"))
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;
        Ok(block.header)
    }
}

impl ProvingOp for EvmEeOperator {
    type Prover = EvmEeProver;
    type Params = (Buf32, Buf32);

    async fn create_task(
        &self,
        block_range: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let (start_blkid, end_blkid) = block_range;
        let context = ProofContext::EvmEeStf(start_blkid, end_blkid);

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(context, vec![], db)
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<EvmEeProofInput, ProvingTaskError> {
        let (start_block_hash, end_block_hash) = match task_id.context() {
            ProofContext::EvmEeStf(start, end) => (*start, *end),
            _ => return Err(ProvingTaskError::InvalidInput("EvmEe".to_string())),
        };

        let mut mini_batch = Vec::new();

        let mut blkid = end_block_hash;
        loop {
            let witness: EvmBlockStfInput = self
                .el_client
                .request("strataee_getBlockWitness", rpc_params![blkid, true])
                .await
                .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

            mini_batch.push(witness);

            if blkid == start_block_hash {
                break;
            } else {
                blkid = Buf32::from(
                    self.get_block_header(blkid)
                        .await
                        .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
                        .parent_hash,
                );
            }
        }
        mini_batch.reverse();

        Ok(mini_batch)
    }
}
