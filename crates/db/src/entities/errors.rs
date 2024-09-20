use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum EntityError {
    #[error("failed to handle bridge signature due to {0}")]
    BridgeTxState(#[from] BridgeTxStateError),
}

pub type EntityResult<T> = Result<T, EntityError>;

#[derive(Debug, Clone, Error)]
pub enum BridgeTxStateError {
    #[error("signer is not part of the assigned operators")]
    Unauthorized,
}
