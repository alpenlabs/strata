//! Key derivation for Strata sequencer

use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv, Xpub};
use secp256k1::SECP256K1;
#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::KeyError;

/// The Strata base index for sequencer keys.
const BASE_IDX: u32 = 56;

/// The Strata sequencer index.
const SEQUENCER_IDX: u32 = 10;

/// The Strata sequencer's master, and derived keys.
#[derive(Debug, Clone)]
pub struct SequencerKeys {
    /// Sequencer's master [`Xpriv`].
    master: Xpriv,

    /// Sequencer's derived [`Xpriv`].
    derived: Xpriv,
}

impl SequencerKeys {
    /// Creates a new [`SequencerKeys`] from a master [`Xpriv`].
    pub fn new(master: &Xpriv) -> Result<Self, KeyError> {
        let path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(BASE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(SEQUENCER_IDX).unwrap(),
        ]);

        let derived = master.derive_priv(SECP256K1, &path)?;
        Ok(Self {
            master: *master,
            derived,
        })
    }

    /// Sequencer's master [`Xpriv`].
    pub fn master(&self) -> &Xpriv {
        &self.master
    }

    /// Sequencer's derived [`Xpriv`].
    pub fn derived(&self) -> &Xpriv {
        &self.derived
    }

    /// Sequencer's master [`Xpub`].
    ///
    /// Infallible conversion from [`Xpriv`] to [`Xpub`] according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn master_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.master)
    }

    /// Sequencer's derived [`Xpub`].
    ///
    /// Infallible conversion from [`Xpriv`] to [`Xpub`] according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn derived_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.derived)
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for SequencerKeys {
    fn zeroize(&mut self) {
        let Self { master, derived } = self;

        // # Security note
        //
        // Going over all possible "zeroizable" fields.
        // What we cannot zeroize is:
        //
        // - Network
        // - Child number
        //
        // These are fine to leave as they are since they are public parameters,
        // and not secret values.

        master.depth.zeroize();
        let mut parent_fingerprint: [u8; 4] = *master.parent_fingerprint.as_mut();
        parent_fingerprint.zeroize();
        master.private_key.non_secure_erase();
        let mut chaincode: [u8; 32] = *master.chain_code.as_mut();
        chaincode.zeroize();

        derived.depth.zeroize();
        let mut parent_fingerprint: [u8; 4] = *derived.parent_fingerprint.as_mut();
        parent_fingerprint.zeroize();
        derived.private_key.non_secure_erase();
        let mut chaincode: [u8; 32] = *derived.chain_code.as_mut();
        chaincode.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl ZeroizeOnDrop for SequencerKeys {}
