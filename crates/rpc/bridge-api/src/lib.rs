//! Provides bridge-related APIs for the RPC server.
//!
//! Provides high-level traits that form the RPC interface of the Bridge. The RPCs have been
//! decomposed into various groups partly based on how bitcoin RPCs are categorized into various
//! [groups](https://developer.bitcoin.org/reference/rpc/index.html).

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait StrataBridgeControlApi {
    /// Get the bridge protocol version.
    #[method(name = "getProtocolVersion")]
    async fn get_client_version(&self) -> RpcResult<String>;

    /// Get the uptime for the client in seconds assuming the clock is strictly monotonically
    /// increasing.
    #[method(name = "uptime")]
    async fn get_uptime(&self) -> RpcResult<u64>;
}

/// RPCs related to network information including healthcheck, node addresses, etc.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait StrataBridgeNetworkApi {
    /// Request to send a `ping` to all other nodes.
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<()>;
}
