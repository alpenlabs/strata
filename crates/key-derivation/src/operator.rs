//! Key derivation for bridge operators
//!
//! Bridge operators guarantee the security assumptions of the Strata BitVM-based
//! bridge by enforcing that all peg-ins and peg-outs are valid.
//!
//! Operators are responsible for their own keys and master [`Xpriv`](bitcoin::bip32::Xpriv) is not
//! shared between operators. Hence, this crate has a BYOK (Bring Your Own Key) design.
//!
//! They use a set of keys to sign messages and bitcoin transactions.
//! The keys are derived from a master [`Xpriv`](bitcoin::bip32::Xpriv)
//! using a [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
//! HD derivation path.
//!
//! The derivation paths are:
//!
//! - `m/56'/20'/100'` for the message signing key
//! - `m/56'/20'/101'` for the wallet transaction signing key
//!
//! These follow the [BIP-44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
//! levels for bitcoin keys:
//!
//! - `m/56'` is the BIP-44 purpose level
//! - `m/56'/20'` is the coin type level
//! - `m/56'/20'/100'` is the account level
//!
//! These are all hardened child indices, allowing extra security if an operator derived
//! [`Xpub`](bitcoin::bip32::Xpub) is compromised.

use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    secp256k1::SECP256K1,
    XOnlyPublicKey,
};
use miniscript::{descriptor::Tr, Descriptor};

use crate::error::KeyError;

/// The BIP-44 purpose index for operator keys.
const PURPOSE_IDX: u32 = 56;

/// The BIP-44 coin type index for operator keys.
const COIN_TYPE_IDX: u32 = 20;

/// The BIP-44 account index for operator keys.
const ACCOUNT_MESSAGE_IDX: u32 = 100;

/// The BIP-44 account index for the operator wallet key.
const ACCOUNT_WALLET_IDX: u32 = 101;

/// The operator's message signing and wallet transaction signing keys.
///
/// The keys have different [`Xpriv`] derivation paths to allow for different
/// key lifetimes, while adding some security against a leaked signing key.
#[derive(Debug, Clone)]
pub struct OperatorKeys {
    /// The operator's message signing key.
    signing: Xpriv,
    /// The operator's wallet transaction signing key.
    wallet: Xpriv,
}

impl OperatorKeys {
    /// Creates a new [`OperatorKeys`] from a master [`Xpriv`].
    pub fn new(master: &Xpriv) -> Result<Self, KeyError> {
        let message_path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(PURPOSE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(COIN_TYPE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(ACCOUNT_MESSAGE_IDX).unwrap(),
        ]);

        let wallet_path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(PURPOSE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(COIN_TYPE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(ACCOUNT_WALLET_IDX).unwrap(),
        ]);

        let message_xpriv = master.derive_priv(SECP256K1, &message_path)?;
        let wallet_xpriv = master.derive_priv(SECP256K1, &wallet_path)?;

        Ok(OperatorKeys {
            signing: message_xpriv,
            wallet: wallet_xpriv,
        })
    }

    /// The operator's message signing [`Xpub`].
    ///
    /// Infallible according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn message_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.signing)
    }

    /// The operator's wallet transaction signing [`Xpub`].
    ///
    /// Infallible conversion from [`Xpriv`] to [`Xpub`] according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn wallet_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.wallet)
    }

    /// The operator's wallet descriptor.
    pub fn wallet_descriptor(&self) -> Descriptor<XOnlyPublicKey> {
        // use the `Tr` type to create a Taproot descriptor
        todo!()
    }

    /// The operator's message descriptor.
    pub fn message_descriptor(&self) -> Descriptor<XOnlyPublicKey> {
        // use the `Tr` type to create a Taproot descriptor
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_keys() {
        // get nice xprivs from rust-bitcoin tests cases
        todo!()
    }

    #[test]
    fn test_operator_descriptors() {
        // get nice xprivs from rust-bitcoin tests cases
        todo!()
    }
}
