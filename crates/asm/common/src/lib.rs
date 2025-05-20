//! The crate provides common types and traits for building blocks for defining
//! and interacting with subprotocols in an ASM (Anchor State Machine) framework.

mod error;
mod msg;
mod spec;
mod state;
mod subprotocol;

pub use error::AsmError;
pub use msg::{InterprotoMsg, Log, NullMsg};
pub use spec::{AsmSpec, Stage};
pub use state::{AnchorState, ChainViewState, SectionState};
pub use subprotocol::{MsgRelayer, Subprotocol, SubprotocolId, TxInput};
