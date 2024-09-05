//! Build bitcoin scripts.
//!
//! Handles creation of bitcoin scripts via `bitcoin-rs`. Provides high-level APIs to get
//! fully-formed bridge-related scripts.

use bitcoin::{taproot::ControlBlock, ScriptBuf, Transaction, TxOut};
use builder::TxBuilder;
use errors::BridgeTxBuilderResult;

pub mod builder;
pub mod deposit;
pub mod errors;
pub mod prelude;
pub mod withdrawal;

/// Trait for any (bridge) transaction.
///
/// This is implemented by any struct that contains bridge-specific information to create
/// transactions.
pub trait TxKind {
    /// Computes the witness elements required to spend the inputs in order (except the signatures).
    fn compute_spend_infos(
        &self,
        builder: &TxBuilder,
    ) -> BridgeTxBuilderResult<Vec<(ScriptBuf, ControlBlock)>>;

    /// Computes the prevouts required to sign a taproot transaction.
    fn compute_prevouts(&self, builder: &TxBuilder) -> BridgeTxBuilderResult<Vec<TxOut>>;

    /// Create the transaction with the help of a [`TxBuilder`].
    fn create_unsigned_tx(&self, builder: &TxBuilder) -> BridgeTxBuilderResult<Transaction>;
}
