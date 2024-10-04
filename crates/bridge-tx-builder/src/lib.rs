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

use context::BuildContext;
use errors::BridgeTxBuilderResult;
use strata_primitives::bridge::TxSigningData;

/// Defines a method that any bridge transaction must implement in order to create a
/// structure that can be signed.
///
/// This is implemented by any struct that contains bridge-specific information to create
/// transactions.
pub trait TxKind {
    /// Create the [`TxSigningData`] required to create the final signed transaction.
    fn construct_signing_data<C: BuildContext>(
        &self,
        build_context: &C,
    ) -> BridgeTxBuilderResult<TxSigningData>;
}
