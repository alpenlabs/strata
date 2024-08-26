//! Errors during parsing/handling/conversion of primitives.

use bitcoin::{address, secp256k1};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum BridgeParseError {
    #[error("public key table should be sorted by the `OperatorIdx`")]
    MalformedPublicKeyTable,

    #[error("supplied pubkey is invalid")]
    InvalidPubkey(#[from] secp256k1::Error),

    #[error("supplied address is invalid")]
    InvalidAddress(#[from] address::ParseError),

    #[error("only taproot addresses are supported")]
    UnsupportedAddress,
}
