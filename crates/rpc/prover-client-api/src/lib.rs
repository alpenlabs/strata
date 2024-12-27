//! Provides prover-client related APIs for the RPC server.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use strata_primitives::{buf::Buf32, l2::L2BlockId};
use strata_rpc_types::ProofKey;
use strata_state::l1::L1BlockId;
use strata_zkvm::ProofReceipt;

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev_strata"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "dev_strata"))]
pub trait StrataProverClientApi {
    /// Start proving the given btc block
    #[method(name = "proveBtcBlock")]
    async fn prove_btc_block(&self, block_id: L1BlockId) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given el block
    #[method(name = "proveElBlocks")]
    async fn prove_el_blocks(&self, el_block_range: (Buf32, Buf32)) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given cl block
    #[method(name = "proveClBlocks")]
    async fn prove_cl_blocks(
        &self,
        cl_block_range: (L2BlockId, L2BlockId),
    ) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given l1 Batch
    #[method(name = "proveL1Batch")]
    async fn prove_l1_batch(&self, l1_range: (L1BlockId, L1BlockId)) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given l2 batch
    #[method(name = "proveL2Batch")]
    async fn prove_l2_batch(
        &self,
        l2_range: Vec<(L2BlockId, L2BlockId)>,
    ) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given checkpoint info
    #[method(name = "proveCheckpointRaw")]
    async fn prove_checkpoint_raw(
        &self,
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
    ) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the given checkpoint
    #[method(name = "proveCheckpoint")]
    async fn prove_checkpoint(&self, ckp_idx: u64) -> RpcResult<Vec<ProofKey>>;

    /// Start proving the latest checkpoint info from the sequencer
    #[method(name = "proveLatestCheckPoint")]
    async fn prove_latest_checkpoint(&self) -> RpcResult<Vec<ProofKey>>;

    /// Get the task status of `key`
    #[method(name = "getTaskStatus")]
    async fn get_task_status(&self, key: ProofKey) -> RpcResult<Option<String>>;

    /// Get proof with the given `key`
    #[method(name = "getProof")]
    async fn get_proof(&self, key: ProofKey) -> RpcResult<Option<ProofReceipt>>;
}
