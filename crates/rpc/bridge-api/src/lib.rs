//! Provides bridge-related APIs for the RPC server.
//!
//! Provides high-level traits that form the RPC interface of the Bridge. The RPCs have been
//! decomposed into various groups partly based on how bitcoin RPCs are categorized into various
//! [groups](https://developer.bitcoin.org/reference/rpc/index.html).

use bitcoin::secp256k1::schnorr::Signature;
use express_bridge_txm::DepositInfo;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

pub mod services;

/// RPCs related to information about the client itself.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeControlApi {
    /// Get the bridge protocol version.
    #[method(name = "getProtocolVersion")]
    async fn get_protocol_version(&self) -> RpcResult<String>;

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

/// RPCS related to the wallet-related functionalities mainly signing and verifying.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeWalletApi {
    /// Request the signature for a Deposit TX.
    #[method(name = "requestSignature")]
    async fn request_signature(&self, deposit_info: DepositInfo) -> RpcResult<Signature>;

    /// Get the signature for the deposit transaction given a bitcoin address.
    #[method(name = "signDepositTransaction")]
    async fn sign_deposit_transaction(
        &self,
        address: String,
        deposit_info: DepositInfo,
    ) -> RpcResult<Signature>;

    /// Verify that the provided signature is correct for the given params.
    #[method(name = "verifyTransaction")]
    async fn verify_deposit_transaction(
        &self,
        address: String,
        deposit_info: DepositInfo,
        signature: Signature,
    ) -> RpcResult<()>;
}
