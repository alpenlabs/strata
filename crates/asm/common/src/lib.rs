//! The crate provides common types and traits for building blocks for defining
//! and interacting with subprotocols in an ASM (Anchor State Machine) framework.

mod error;
mod msg;
mod state;
mod subprotocol;

pub use error::ASMError;
pub use msg::*;
pub use state::*;
pub use subprotocol::Subprotocol;
