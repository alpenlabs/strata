//! Defines errors associated with the signature manager.

use alpen_express_db::{entities::errors::EntityError, DbError};
use bitcoin::{psbt::ExtractTxError, sighash::TaprootError};
use musig2::errors::{KeyAggError, SigningError, VerifyError};
use thiserror::Error;

/// Errors that may occur during the signing and aggregation of signatures for a particular
/// [`Psbt`](bitcoin::Psbt).
#[derive(Debug, Clone, Error)]
pub enum BridgeSigError {
    /// Failed to build a [`Psbt`] from the unsigned transaction. This can happen if the
    /// transaction that is being converted to a psbt contains a non-empty script sig or
    /// witness fields.
    #[error("failed to build psbt: {0}")]
    BuildPsbtFailed(String),

    /// No input exists for the given index in the psbt.
    #[error("no input exists for the given index in the PSBT")]
    InputIndexOutOfBounds,

    /// The provided signature is not valid for the given transaction and pubkey.
    #[error("signature validation failed due to: {0}")]
    InvalidSignature(#[from] VerifyError),

    /// The pubkey is not part of the signatories required for the psbt.
    #[error("pubkey is not a required signatory")]
    UnauthorizedPubkey,

    /// Problem with key aggregation.
    #[error("Encountered a problem during key aggregation: {0}")]
    KeyAggError(#[from] KeyAggError),

    /// Error occurred while persisting/accessing signatures.
    #[error("could not persist/access entity due to: {0}")]
    StorageError(#[from] DbError),

    /// Error occurred while persisting/accessing signatures.
    #[error("invalid operation on entity: {0}")]
    EntityError(#[from] EntityError),

    /// Transaction for the provided txid does not exist in state/storage.
    #[error("transaction does not exist")]
    TransactionNotFound,

    /// Transaction for the provided txid already exists in state/storage.
    #[error("transaction already exists in the persistence layer")]
    DuplicateTransaction,

    /// Not all required nonces from the MuSig2 participants have been collected.
    #[error("not all nonces have been collected yet")]
    IncompleteNonces,

    /// The transaction is not fully signed yet.
    #[error("transaction not fully signed yet")]
    NotFullySigned,

    /// The witness stack in the transaction does not contain the script and control block.
    #[error("initial witness block cannot be empty")]
    EmptyWitnessBlock,

    /// Failed to create signed transaction after all signatures have been collected.
    #[error("failed to build signed transaction due to {0}")]
    BuildSignedTxFailed(#[from] ExtractTxError),

    /// Failed to finalize a [`bitcoin::Psbt`].
    #[error("could not finalize psbt due to: {0:?}")]
    PsbtFinalizationError(String),

    /// Failed to produce taproot sig hash
    #[error("failed to create taproot sig hash due to {0}")]
    SighashError(#[from] TaprootError),

    /// Issue while producing a partial MuSig2 signature.
    #[error("failed to generate partial signature due to: {0}")]
    PartialSigningFailed(#[from] SigningError),
}

/// Result type alias for the signature manager with [`BridgeSigError`] as the Error variant.
pub type BridgeSigResult<T> = Result<T, BridgeSigError>;
