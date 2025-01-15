use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DepositParseError {
    #[error("No OP_RETURN")]
    NoOpReturn,

    #[error("No data")]
    NoData,

    #[error("no magic bytes")]
    NoMagicBytes,

    #[error("magic bytes mismatch")]
    MagicBytesMismatch,

    #[error("no address found")]
    NoDestAddress,

    #[error("invalid destination Address length {0}")]
    InvalidDestAddress(u8),

    #[error("unexpected amount (exp {0}, found {1}) ")]
    ExpectedAmount(u64, u64),

    #[error("no leaf hash found")]
    NoLeafHash,

    #[error("expected 32 byte leaf Hash")]
    LeafHashLenMismatch,

    #[error("no taproot script")]
    NoP2TR,
}
