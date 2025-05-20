//! The crate provides common types and traits for building blocks for defining
//! and interacting with subprotocols in an ASM (Anchor State Machine) framework.

// TODO figure this out
use bitcoin_bosd::Descriptor;

mod error;
mod msg;
mod state;
mod subprotocol;

pub use error::AsmError;
pub use msg::{InterprotoMsg, Log, NullMsg};
pub use state::{AnchorState, ChainViewState, SectionState};
pub use subprotocol::{MsgRelayer, Subprotocol, SubprotocolId, TxInput};
