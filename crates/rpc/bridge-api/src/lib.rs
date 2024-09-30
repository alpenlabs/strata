//! Provides bridge-related APIs for the RPC server.
//!
//! Provides high-level traits that form the RPC interface of the Bridge. The RPCs have been
//! decomposed into various groups partly based on how bitcoin RPCs are categorized into various
//! [groups](https://developer.bitcoin.org/reference/rpc/index.html).

use alpen_express_state::bridge_duties::BridgeDutyStatus;
use bitcoin::Txid;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeControlApi {
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
pub trait ExpressBridgeNetworkApi {
    /// Request to send a `ping` to all other nodes.
    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<()>;
}

/// RPCs related to the tracking of information regarding various duties.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeTrackerApi {
    /// Get the status of the bridge duty associated with a particular [`Txid`].
    #[method(name = "getDutyStatus")]
    async fn get_status(&self, txid: Txid) -> RpcResult<Option<BridgeDutyStatus>>;

    // TODO: add other duty RPCs as necessary (for example, `pendingDuties`, `executedDuties`, etc.)
}
