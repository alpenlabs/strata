//! Build bitcoin scripts.
//!
//! Handles creation of bitcoin scripts via `bitcoin-rs`. Provides high-level APIs to get
//! fully-formed bridge-related scripts.

pub mod constants;
pub mod context;
pub mod deposit;
pub mod errors;
pub mod operations;
pub mod prelude;
pub mod withdrawal;

use alpen_express_primitives::bridge::TxSigningData;
use context::BuilderContext;
use errors::BridgeTxBuilderResult;

/// Trait that defines a method that any bridge transaction must implement in order to create a
/// structure that can be signed.
///
/// This is implemented by any struct that contains bridge-specific information to create
/// transactions.
pub trait TxKind {
    /// The cryptographic context required to build the transaction.
    type Context: BuilderContext;

    /// Create the [`TxSigningData`] required to create the final signed transaction.
    fn construct_signing_data(
        &self,
        builder: &Self::Context,
    ) -> BridgeTxBuilderResult<TxSigningData>;
}
