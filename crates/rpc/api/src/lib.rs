//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.

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

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct ClientStatus {
    /// Blockchain tip.
    pub chain_tip: String,

    /// L2 block that's been finalized and proven on L1.
    pub finalized_blkid: String,

    /// Recent L1 blocks that we might still reorg.
    pub last_l1_block: String,

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

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<L1Status>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<ClientStatus>;
}
