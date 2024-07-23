#![allow(unexpected_cfgs)] // TODO: remove this when we add the `client` feature flag.
//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct L1Status {
    /// If the last time we tried to poll the client (as of `last_update`)
    /// we were successful.
    pub bitcoin_rpc_connected: bool,

    /// The last error message we received when trying to poll the client, if
    /// there was one.
    pub last_rpc_error: Option<String>,

    /// Current block height.
    pub cur_height: u64,

    /// Current tip block ID as string.
    pub cur_tip_blkid: String,

    /// UNIX millis time of the last time we got a new update from the L1 connector.
    pub last_update: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientStatus {
    /// L1 blockchain tip.
    #[serde(with = "hex::serde")]
    pub chain_tip: [u8; 32],

    /// L1 chain tip slot.
    pub chain_tip_slot: u64,

    /// L2 block that's been finalized and proven on L1.
    #[serde(with = "hex::serde")]
    pub finalized_blkid: [u8; 32],

    /// Recent L1 block that we might still reorg.
    #[serde(with = "hex::serde")]
    pub last_l1_block: [u8; 32],

    /// L1 block index we treat as being "buried" and won't reorg.
    pub buried_l1_height: u64,
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alp"))]
pub trait AlpenApi {
    // TODO the rest of these
    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    // TODO make this under the admin RPC interface
    #[method(name = "stop")]
    async fn stop(&self) -> RpcResult<()>;

    #[method(name = "l1connected")]
    async fn get_l1_connection_status(&self) -> RpcResult<bool>;

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<L1Status>;

    #[method(name = "getL1blockHash")]
    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<String>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<ClientStatus>;
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "test"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "test"))]
pub trait AlpenFuncTestApi {
    #[method(name = "postBlob")]
    /// Basically adds L1Write sequencer duty which will be executed by sequencer
    async fn trigger_da_blob(&self, blobdata: Vec<u8>) -> RpcResult<()>;

    #[method(name = "testFunctional")]
    /// Basically adds L1Write sequencer duty which will be executed by sequencer
    async fn test_functional(&self) -> RpcResult<u32>;
}
