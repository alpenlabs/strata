use bitcoin::{psbt, Txid};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum EntityError {
    #[error("failed to handle bridge signature due to {0}")]
    BridgeTxStateError(#[from] BridgeTxStateError),

    #[error("signer is not part of the assigned operators")]
    BridgeOpUnauthorized,
}

pub type EntityResult<T> = Result<T, EntityError>;

#[derive(Debug, Clone, Error)]
pub enum BridgeTxStateError {
    #[error("bridge tx {0} has no input at index {1} to add signature to")]
    TxinIdxOutOfBounds(Txid, usize),

    #[error("signer is not part of the assigned operators")]
    Unauthorized,

    #[error("encountered problem with psbt due to: {0}")]
    PsbtError(String),
}

/// Manual implementation of conversion for [`psbt::Error`] <-> [`BridgeSigEntityError`] as the
/// former does not implement [`Clone`] ¯\_(ツ)_/¯.
impl From<psbt::Error> for BridgeTxStateError {
    fn from(value: psbt::Error) -> Self {
        Self::PsbtError(value.to_string())
    }
}
