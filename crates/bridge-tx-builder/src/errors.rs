//! Enumerated errors related to creation and signing of bridge-related transactions.

use bitcoin::taproot::{TaprootBuilder, TaprootBuilderError};
use thiserror::Error;

/// Error during building of bridge-related transactions.
#[derive(Debug, Error)]
pub enum BridgeTxBuilderError {
    /// Error building the Deposit Transaction.
    #[error("could not build deposit transaction")]
    DepositTransaction(#[from] DepositTransactionError),

    /// Error due to there being no script provided to create a taproot address.
    #[error("noscript taproot address without an internal key not supported")]
    EmptyTapscript,

    /// Error while building the taproot address.
    #[error("could not build taproot address")]
    BuildFailed(#[from] TaprootBuilderError),

    /// Error while adding a leaf to to a [`TaprootBuilder`].
    #[error("could not add leaf to the taproot tree")]
    CouldNotAddLeaf,

    /// An unexpected error occurred.
    // HACK: This should only be used while developing, testing or bikeshedding the right variant
    // for a particular error.
    #[error("unexpected error occurred: {0}")]
    Unexpected(String),
}

/// Result type alias that has [`BridgeTxBuilderError`] as the error type for succinctness.
pub type BridgeTxBuilderResult<T> = Result<T, BridgeTxBuilderError>;

/// The unmodified [`TaprootBuilder`] is returned if a leaf could not be added to the taproot in the
/// call to [`TaprootBuilder::add_leaf`].
impl From<TaprootBuilder> for BridgeTxBuilderError {
    fn from(_value: TaprootBuilder) -> Self {
        BridgeTxBuilderError::CouldNotAddLeaf
    }
}

/// Error building the Deposit Transaction.
#[derive(Debug, Error)]
pub enum DepositTransactionError {
    /// Invalid address provided in the Deposit Request Transaction output.
    #[error("invalid deposit request taproot address")]
    InvalidDRTAddress,

    /// Invalid address size provided for the execution layer address where the bridged-in amount
    /// is to be minted.
    #[error("el size exceeds expected size: {0} > 20")]
    InvalidElAddressSize(usize),

    /// Error while generating the control block. This mostly means that the control block is
    /// invalid i.e., it does not have the right commitment.
    #[error("control block generation invalid")]
    ControlBlockError,

    /// The provided tapleaf hash (merkle branch) is invalid.
    #[error("invalid merkle proof")]
    InvalidTapLeafHash,
}
