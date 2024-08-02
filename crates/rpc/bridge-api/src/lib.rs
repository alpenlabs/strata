//! Provides bridge-related APIs for the RPC server.
//!
//! Provides high-level traits that form the RPC interface of the Bridge. The RPCs have been
//! decomposed into various groups partly based on how bitcoin RPCs can be categorized into various
//! [groups](https://developer.bitcoin.org/reference/rpc/index.html).

use bitcoin::secp256k1::{schnorr::Signature, SecretKey};
use express_bridge_txm::DepositInfo;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeNetworkApi {
    /// Get the bridge protocol version.
    #[method(name = "getProtocolVersion")]
    async fn get_protocol_version(&self) -> RpcResult<String>;

    /// Request the signature for a Deposit TX.
    #[method(name = "requestSignature")]
    async fn request_signature(&self, deposit_info: DepositInfo) -> RpcResult<Signature>;
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeWalletApi {
    /// Get the signature for the deposit transaction given a SecretKey.
    #[method(name = "signDepositTransaction")]
    async fn sign_deposit_transaction(
        &self,
        privkey: SecretKey,
        deposit_info: DepositInfo,
    ) -> RpcResult<Signature>;
}
