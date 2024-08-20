//! Manage bridge-related transactions.
//!
//! Provides useful abstractions over `bitcoin-rs` for creating, signing and storing
//! bitcoin transactions relevant to the Bridge. It also stores signatures for various pre-signed
//! transactions and allows looking them up when necessary (for example, when other component or
//! nodes request for them).

pub mod signature_manager;
pub mod tx_builder;

// Re-exports
pub use signature_manager::*;
pub use tx_builder::*;
