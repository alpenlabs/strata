#![allow(unexpected_cfgs)] // TODO: remove this when we add the `client` feature flag.
//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_rpc_types::types::{
    BlockHeader, ClientStatus, DepositEntry, ExecUpdate, L1Status,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

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

    #[method(name = "getRecentBlocks")]
    async fn get_recent_blocks(&self, count: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getBlocksAtIdx")]
    async fn get_blocks_at_idx(&self, index: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getBlockById")]
    async fn get_block_by_id(&self, block_id: String) -> RpcResult<BlockHeader>;

    #[method(name = "getExecUpdateById")]
    async fn get_exec_update_by_id(&self, block_id: String) -> RpcResult<ExecUpdate>;

    #[method(name = "getCurrentDeposits")]
    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>>;

    #[method(name = "getCurrentDepositById")]
    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<DepositEntry>;
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct HexBytes(#[serde_as(as = "serde_with::hex::Hex")] pub Vec<u8>);

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpadmin"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpadmin"))]
pub trait AlpenAdminApi {
    #[method(name = "submitDABlob")]
    /// Basically adds L1Write sequencer duty which will be executed by sequencer
    async fn submit_da_blob(&self, blobdata: HexBytes) -> RpcResult<()>;
}
