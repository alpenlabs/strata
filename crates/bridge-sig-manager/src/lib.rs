//! Handles signing and storing of signatures.
//!
//! Provides APIs to sign the given transaction based on the configured `Reserved Address`
//! or private key, store the signatures and look them up when necessary.

pub mod errors;
pub mod prelude;
pub mod signature;
pub mod state;
