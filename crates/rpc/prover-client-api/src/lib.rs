//! Provides prover-client related APIs for the RPC server.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use strata_rpc_types as _;
use uuid::Uuid;

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev_strata"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "dev_strata"))]
pub trait StrataProverClientApi {
    /// Start proving the given btc block
    #[method(name = "proveBtcBlock")]
    async fn prove_btc_block(&self, el_block_num: u64) -> RpcResult<Uuid>;

    /// Start proving the given el blocks
    #[method(name = "proveElBlocks")]
    async fn prove_el_blocks(&self, block_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given cl blocks
    #[method(name = "proveCLBlocks")]
    async fn prove_cl_block(&self, block_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given l1 Batch
    #[method(name = "proveL1Batch")]
    async fn prove_l1_batch(&self, l1_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given l2 batch
    #[method(name = "proveL2Batch")]
    async fn prove_l2_batch(&self, l2_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given checkpoint info
    #[method(name = "proveCheckpointRaw")]
    async fn prove_checkpoint_raw(
        &self,
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
    ) -> RpcResult<Uuid>;

    /// Start proving the latest checkpoint info from the sequencer
    #[method(name = "proveLatestCheckPoint")]
    async fn prove_latest_checkpoint(&self) -> RpcResult<Uuid>;

    /// Start proving the given el block
    #[method(name = "getTaskStatus")]
    async fn get_task_status(&self, task_id: Uuid) -> RpcResult<Option<String>>;
}
