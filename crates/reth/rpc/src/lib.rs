//! alpen custom reth rpc

mod rpc;

use reth_primitives::B256;
pub use rpc::AlpenRPC;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::{Deserialize, Serialize};
use zkvm_primitives::ZKVMInput;

#[cfg_attr(not(test), rpc(server, namespace = "alpen"))]
#[cfg_attr(test, rpc(server, client, namespace = "alpen"))]
pub trait AlpenRpcApi {
    /// Returns the state changesets for requested blocks.
    #[method(name = "blockWitness")]
    fn block_witness(
        &self,
        block_hash: B256,
        json: Option<bool>,
    ) -> RpcResult<Option<BlockWitness>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockWitness {
    Raw(#[serde(with = "hex::serde")] Vec<u8>),
    Json(ZKVMInput),
}
