//! Error types for key derivation.

use std::fmt::Display;

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum KeyError {
    /// An error from the [`bitcoin::bip32`] module.
    ///
    /// This means that the [`Xpriv`](bitcoin::bip32::Xpriv)
    /// is not a valid extended private key or the
    /// [`DerivationPath`](bitcoin::bip32::DerivationPath)
    /// is invalid.
    Bip32Error(#[from] bitcoin::bip32::Error),
}

impl Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
