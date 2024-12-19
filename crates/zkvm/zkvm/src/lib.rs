use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

mod env;
mod errors;
mod host;
mod input;
mod proof;
mod prover;

pub use env::*;
pub use errors::*;
pub use host::*;
pub use input::*;
pub use proof::*;
pub use prover::*;

/// Represents the ZkVm host used for proof generation.
///
/// This enum identifies the ZkVm environment utilized to create a proof.
/// Available hosts:
/// - `SP1`: SP1 ZKVM.
/// - `Risc0`: Risc0 ZKVM.
/// - `Native`: Native ZKVM.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
)]
pub enum ZkVm {
    SP1,
    Risc0,
    Native,
}
