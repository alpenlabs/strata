use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DepositParseError {
    #[error("missing tag")]
    MissingTag,

    /// What is this used for?
    #[error("no data")]
    NoData,

    /// We don't accept nonstandard deposit things.
    #[error("tag too large")]
    TagOversized,

    #[error("missing magic bytes")]
    MissingMagic,

    #[error("invalid magic bytes")]
    InvalidMagic,

    #[error("missing destination")]
    MissingDest,

    #[error("invalid destination length {0}")]
    InvalidDestLen(u8),

    #[error("unexpected amount (exp {0}, found {1}) ")]
    UnexpectedAmt(u64, u64),

    #[error("no leaf hash found")]
    NoLeafHash,

    #[error("expected 32 byte leaf Hash")]
    LeafHashLenMismatch,

    /// Previously called "NoP2TR" which was really ambiguous.
    #[error("invalid deposit output")]
    InvalidDepositOutput,
}
