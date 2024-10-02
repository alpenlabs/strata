//! Provides prover-client related APIs for the RPC server.

use alpen_express_rpc_types::RpcCheckpointInfo;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use uuid::Uuid;

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev_alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "dev_alp"))]
pub trait ExpressProverClientApi {
    /// Start proving the given el block
    #[method(name = "proveBtcBlock")]
    async fn prove_btc_block(&self, el_block_num: u64) -> RpcResult<Uuid>;

    /// Start proving the given el block
    #[method(name = "proveELBlock")]
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<Uuid>;

    /// Start proving the given cl block
    #[method(name = "proveCLBlock")]
    async fn prove_cl_block(&self, cl_block_num: u64) -> RpcResult<Uuid>;

    /// Start proving the given cl block
    #[method(name = "proveL1Batch")]
    async fn prove_l1_batch(&self, l1_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given cl batch
    #[method(name = "proveL2Batch")]
    async fn prove_l2_batch(&self, l2_range: (u64, u64)) -> RpcResult<Uuid>;

    /// Start proving the given cl block
    #[method(name = "proveCheckpoint")]
    async fn prove_checkpoint(&self, checkpoint: RpcCheckpointInfo) -> RpcResult<Uuid>;

    /// Start proving the given el block
    #[method(name = "getTaskStatus")]
    async fn get_task_status(&self, task_id: Uuid) -> RpcResult<Option<String>>;
}
