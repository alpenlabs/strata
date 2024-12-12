//! Error types for key derivation.

use std::fmt::{Debug, Display, Formatter, Result};

use bitcoin::bip32;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum KeyError {
    /// An error from the [`bip32`] module.
    ///
    /// This means that the [`Xpriv`](bip32::Xpriv)
    /// is not a valid extended private key or the
    /// [`DerivationPath`](bip32::DerivationPath)
    /// is invalid.
    Bip32Error(#[from] bip32::Error),
}

impl Display for KeyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}
