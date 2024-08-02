//! Manage bridge-related transactions.
//!
//! Provides useful abstractions over `bitcoin-rs` for creating, signing and storing
//! bitcoin transactions relevant to the Bridge. It also stores signatures for various pre-signed
//! transactions and allows looking them up when necessary (for example, when other component or
//! nodes request for them).

pub mod script_builder;
pub mod signature_handler;

// Re-exports
pub use script_builder::deposit::*;
