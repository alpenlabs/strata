use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DepositParseError {
    #[error("No OP_RETURN")]
    NoOpReturn,

    #[error("No Magic Bytes")]
    NoMagicBytes,

    #[error("Magic bytes mismatch {0:?} != {1:?}")]
    MagicBytesMismatch(Vec<u8>, Vec<u8>),

    #[error("No address found")]
    NoAddress,

    #[error("invalid Destination Address length {0}")]
    InvalidDestAddress(u8),

    #[error("expected amount {0}")]
    ExpectedAmount(u64),

    #[error("No Taproot control block")]
    NoControlBlock,

    #[error("Control block length is not 32")]
    ControlBlockLen,
}
