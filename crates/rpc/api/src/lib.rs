//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "eth"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "eth"))]
pub trait AlpenApi {
    // TODO the rest of these
    #[method(name = "alp_protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    // TODO make this under the admin RPC interface
    #[method(name = "alp_stop")]
    async fn stop(&self) -> RpcResult<()>;
}
