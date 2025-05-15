//! Core types and state transition logic for the Anchor State Machine (ASM).
//!
//! The ASM anchors the Strata orchestration layer to L1, analogous to a rollup smart contract.

mod error;
mod msg;
mod state;
mod stf;
mod subprotocol;

pub use state::*;
pub use stf::*;
pub use subprotocol::*;
