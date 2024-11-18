//! Strata custom reth rpc

mod rpc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_primitives::revm_primitives::alloy_primitives::B256;
pub use rpc::StrataRPC;
use serde::{Deserialize, Serialize};
use strata_proofimpl_evm_ee_stf::ELProofInput;

#[cfg_attr(not(test), rpc(server, namespace = "strataee"))]
#[cfg_attr(test, rpc(server, client, namespace = "strataee"))]
pub trait StrataRpcApi {
    /// Returns the state changesets with storage proofs for requested blocks.
    /// Used as part of input to riscvm during proof generation
    #[method(name = "getBlockWitness")]
    fn get_block_witness(
        &self,
        block_hash: B256,
        json: Option<bool>,
    ) -> RpcResult<Option<BlockWitness>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockWitness {
    Raw(#[serde(with = "hex::serde")] Vec<u8>),
    Json(ELProofInput),
}
