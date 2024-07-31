#![allow(unexpected_cfgs)] // TODO: remove this when we add the `client` feature flag.
//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_primitives::l1::L1Status;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockHeader {
     pub block_idx: u64,
     pub timestamp: u64,
     pub prev_block: [u8;32],
     pub l1_segment_hash: [u8;32],
     pub exec_segment_hash: [u8;32],
     pub state_root: [u8;32],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DepositInfo {

}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecUpdate {

}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecState {

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

    #[method(name = "getRecentBlocks")]
    async fn get_recent_blocks(&self, count: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getBlocksAtIdx")]
    async fn get_blocks_at_idx(&self, index: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getBlockById")]
    async fn get_block_by_id(&self, block_id: String) -> RpcResult<BlockHeader>;

    #[method(name = "getExecUpdateById")]
    async fn get_exec_update_by_id(&self, block_id: String) -> RpcResult<ExecUpdate>;

    #[method(name = "getCurDeposits")]
    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>>;

    #[method(name = "getCurDepositById")]
    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<DepositInfo>;

    #[method(name = "getCurExecState")]
    async fn get_current_exec_state(&self) -> RpcResult<ExecState>;

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
