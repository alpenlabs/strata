//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct L1Status {
    pub bitcoin_rpc_connected: bool,
    pub cur_height: u64,
    pub cur_tip_blkid: String,
    pub last_update: u64,
    pub last_rpc_error: Option<String>,
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

    #[method(name = "l1blockHash")]
    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<String>;
}
