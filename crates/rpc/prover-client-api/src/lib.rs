//! Provides prover-client related APIs for the RPC server.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev_alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "dev_alp"))]
pub trait ExpressProverClientApi {
    /// Start proving the given el block
    #[method(name = "prove_el_block")]
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<String>;
}
