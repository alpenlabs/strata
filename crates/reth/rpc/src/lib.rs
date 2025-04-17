//! Strata custom reth rpc

pub mod eth;
mod rpc;
pub mod sequencer;

pub use eth::{StrataEthApi, StrataNodeCore};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use revm_primitives::{alloy_primitives::B256, Address};
pub use rpc::StrataRPC;
pub use sequencer::SequencerClient;
use serde::{Deserialize, Serialize};
use strata_proofimpl_evm_ee_stf::EvmBlockStfInput;

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
    Json(EvmBlockStfInput),
}

#[derive(Debug, Clone)]
pub enum EOAMode {
    Disabled { allowed_eoa_addrs: Vec<Address> }, // TODO: Maybe use NonEmptyVec like struct
    // one element
    Enabled,
}

impl EOAMode {
    pub fn new(enable: bool, allowed_eoa_addrs: Vec<Address>) -> Self {
        if enable {
            if !allowed_eoa_addrs.is_empty() {
                // TODO: possibly warn, but would be better if this happened around the boundary
            }
            Self::Enabled
        } else {
            Self::Disabled { allowed_eoa_addrs }
        }
    }
}

impl Default for EOAMode {
    fn default() -> Self {
        Self::Disabled {
            allowed_eoa_addrs: Vec::new(),
        }
    }
}
