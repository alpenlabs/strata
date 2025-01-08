//! Errors during parsing/handling/conversion of primitives.

use bitcoin::{address, secp256k1, ScriptBuf};
use thiserror::Error;

use crate::buf::Buf32;

/// Parsing errors that can occur with L1 primitives,
/// such as addresses, pubkeys, and scripts.
#[derive(Debug, Clone, Error)]
pub enum ParseError {
    /// The provided pubkey is invalid.
    #[error("supplied pubkey is invalid")]
    InvalidPubkey(#[from] secp256k1::Error),

    /// The provided address is invalid.
    #[error("supplied address is invalid")]
    InvalidAddress(#[from] address::ParseError),

    /// The provided script is invalid.
    #[error("supplied script is invalid")]
    InvalidScript(#[from] address::FromScriptError),

    /// The provided 32-byte buffer is not a valid point on the curve.
    #[error("not a valid point on the curve: {0}")]
    InvalidPoint(Buf32),

    /// The provided Script is not an `OP_RETURN` script.
    #[error("not an OP_RETURN script: {0}")]
    NotOpReturn(ScriptBuf),

    /// Unsupported withdrawal destination.
    #[error("unsupported withdrawal destination: {0}")]
    UnsupportedDestination(String),
}
