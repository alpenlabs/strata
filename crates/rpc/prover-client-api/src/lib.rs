//! Provides prover-client related APIs for the RPC server.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alp_prover"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alp_prover"))]
pub trait ExpressProverClientApiServer {
    /// Start proving the given el block
    #[method(name = "dev_prove_el_block")]
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<()>;
}
