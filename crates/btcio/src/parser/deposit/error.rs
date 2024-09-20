use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DepositParseError {
    #[error("No OP_RETURN on output {0} of tx")]
    NoOpReturn(u32),

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
}
