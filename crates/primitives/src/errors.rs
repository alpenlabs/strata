//! Errors during parsing/handling/conversion of primitives.

use bitcoin::{address, secp256k1, AddressType};
use thiserror::Error;

use crate::buf::Buf32;

#[derive(Debug, Clone, Error)]
pub enum ParseError {
    #[error("supplied pubkey is invalid")]
    InvalidPubkey(#[from] secp256k1::Error),

    #[error("supplied address is invalid")]
    InvalidAddress(#[from] address::ParseError),

    #[error("only taproot addresses are supported but found {0:?}")]
    UnsupportedAddress(Option<AddressType>),

    #[error("not a valid point on the curve: {0}")]
    InvalidPoint(Buf32),
}
