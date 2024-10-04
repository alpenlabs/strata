//! Provides prover-client related APIs for the RPC server.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "dev_strata"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "dev_strata"))]
pub trait StrataProverClientApi {
    /// Start proving the given el block
    #[method(name = "proveELBlock")]
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<String>;
}
