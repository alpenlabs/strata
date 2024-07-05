//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.

use alpen_vertex_state::{client_state::ClientState, l1::L1BlockId};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct L1Status {
    /// Current block height.
    pub cur_height: u64,

    /// Current tip block ID as string.
    pub cur_tip_blkid: String,

    /// UNIX millis time of the last time we got a new update from the L1 connector.
    pub last_update: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct L2Status {
    /// recent L1 block ID as string
    pub latest_l1_block: String,

    /// finalized L2 block that won't be reorged as string 
    pub finalized_l2_tip: String, 

    /// L1 height that won't be reorged 
    pub buried_l1_height: u64,

    /// number of pending deposits
    pub pending_deposits: u64,

    /// number of pending withdrawals 
    pub pending_withdrawals: u64,

    /// Blocks accepted into rollup chain
    pub accepted_l2_blocks: u64
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

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<L1Status>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<L2Status>;

}
