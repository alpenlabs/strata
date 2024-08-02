//! Provides bridge-related services for the RPC server.
//!
//! Provides a relay layer between the RPC server and the individual components and databases
//! related to the bridge. The RPC server methods call on this server for any bridge-related
//! operations.
use bitcoin::{secp256k1::schnorr::Signature, OutPoint};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_primitives::Address as RollupAddress;
use serde::{Deserialize, Serialize};

/// The metadata associated with a deposit. This will be used to communicated additional
/// information to the rollup. For now, this only carries limited information but we may extend
/// it later.
// TODO: move this to `bridge-tx-manager`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositMetadata {
    /// The protocol version that the deposit is associated with.
    version: String,

    /// Special identifier that helps the `alpen-exrpress-btcio::reader` worker identify relevant
    /// deposits.
    identifier: String,
}

/// The deposit information  required to create the Deposit Transaction.
// TODO: move this to `bridge-tx-manager`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction UTXO from the user.
    deposit_request_utxo: OutPoint,

    /// The rollup address to mint the equivalent tokens to.
    rollup_address: RollupAddress,

    /// The amount in bitcoins that the user wishes to deposit.
    amount_in_sats: u64,

    /// The metadata associated with the deposit request.
    metadata: DepositMetadata,
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
pub trait ExpressBridgeApi {
    /// Get the bridge protocol version.
    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<String>;

    /// Get the signature for a Deposit TX.
    #[method(name = "signDepositTransaction")]
    async fn sign_deposit_tx(&self, deposit_info: DepositInfo) -> RpcResult<Signature>;
}
