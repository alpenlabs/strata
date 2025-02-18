//! Key derivation for Strata sequencer

use bitcoin::bip32::{ChildNumber, Xpriv, Xpub};
use secp256k1::SECP256K1;
use strata_primitives::constants::STRATA_SEQUENCER_DERIVATION_PATH;
#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::KeyError;

/// The Strata sequencer's master, and derived _private_ keys.
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
        let derived = master.derive_priv(SECP256K1, &*STRATA_SEQUENCER_DERIVATION_PATH)?;

        Ok(Self {
            master: *master,
            derived,
        })
    }

    /// Sequencer's master [`Xpriv`].
    pub fn master_xpriv(&self) -> &Xpriv {
        &self.master
    }

    /// Sequencer's derived [`Xpriv`].
    pub fn derived_xpriv(&self) -> &Xpriv {
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
    #[inline]
    fn zeroize(&mut self) {
        let Self { master, derived } = self;

        // # Security note
        //
        // Going over all possible "zeroizable" fields.
        // What we cannot zeroize is only:
        //
        // - Network: enum
        //
        // These are fine to leave as they are since they are public parameters,
        // and not secret values.
        //
        // NOTE: `Xpriv.private_key` (`SecretKey`) `non_secure_erase` writes `1`s to the memory.

        // Zeroize master components
        master.depth.zeroize();
        {
            let fingerprint: &mut [u8; 4] = master.parent_fingerprint.as_mut();
            fingerprint.zeroize();
        }
        master.private_key.non_secure_erase();
        {
            let chaincode: &mut [u8; 32] = master.chain_code.as_mut();
            chaincode.zeroize();
        }
        let raw_ptr = &mut master.child_number as *mut ChildNumber;
        // SAFETY: `master.child_number` is a valid enum variant
        //          and will not be accessed after zeroization.
        //          Also there are only two possible variants that will
        //          always have an `index` which is a `u32`.
        //          Note that `ChildNumber` does not have the `#[non_exhaustive]`
        //          attribute.
        unsafe {
            *raw_ptr = if master.child_number.is_normal() {
                ChildNumber::Normal { index: 0 }
            } else {
                ChildNumber::Hardened { index: 0 }
            };
        }

        // Zeroize derived components
        derived.depth.zeroize();
        {
            let fingerprint: &mut [u8; 4] = derived.parent_fingerprint.as_mut();
            fingerprint.zeroize();
        }
        derived.private_key.non_secure_erase();
        {
            let chaincode: &mut [u8; 32] = derived.chain_code.as_mut();
            chaincode.zeroize();
        }
        let raw_ptr = &mut derived.child_number as *mut ChildNumber;
        // SAFETY: `derived.child_number` is a valid enum variant
        //          and will not be accessed after zeroization.
        //          Also there are only two possible variants that will
        //          always have an `index` which is a `u32`.
        //          Note that `ChildNumber` does not have the `#[non_exhaustive]`
        //          attribute.
        unsafe {
            *raw_ptr = if derived.child_number.is_normal() {
                ChildNumber::Normal { index: 0 }
            } else {
                ChildNumber::Hardened { index: 0 }
            };
        }
    }
}

#[cfg(feature = "zeroize")]
impl ZeroizeOnDrop for SequencerKeys {}

// Manual Drop implementation to zeroize keys on drop.
impl Drop for SequencerKeys {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use bitcoin::Network;
    use strata_primitives::buf::Buf32;
    use strata_sequencer::duty::types::{Identity, IdentityData, IdentityKey};

    use super::*;

    #[test]
    #[cfg(feature = "zeroize")]
    fn test_zeroize() {
        let master = Xpriv::new_master(Network::Regtest, &[2u8; 32]).unwrap();
        let mut keys = SequencerKeys::new(&master).unwrap();

        // Store original values
        let master_chaincode = *keys.master_xpriv().chain_code.as_bytes();
        let derived_chaincode = *keys.derived_xpriv().chain_code.as_bytes();

        // Verify data exists
        assert_ne!(master_chaincode, [0u8; 32]);
        assert_ne!(derived_chaincode, [0u8; 32]);

        // Manually zeroize
        keys.zeroize();

        // Verify fields are zeroed
        // NOTE: SecretKey::non_secure_erase writes `1`s to the memory.
        assert_eq!(keys.master_xpriv().private_key.secret_bytes(), [1u8; 32]);
        assert_eq!(keys.derived_xpriv().private_key.secret_bytes(), [1u8; 32]);
        assert_eq!(*keys.master_xpriv().chain_code.as_bytes(), [0u8; 32]);
        assert_eq!(*keys.derived_xpriv().chain_code.as_bytes(), [0u8; 32]);
        assert_eq!(*keys.master_xpriv().parent_fingerprint.as_bytes(), [0u8; 4]);
        assert_eq!(
            *keys.derived_xpriv().parent_fingerprint.as_bytes(),
            [0u8; 4]
        );
        assert_eq!(keys.master_xpriv().depth, 0);
        assert_eq!(keys.derived_xpriv().depth, 0);

        // Check if child numbers are zeroed while maintaining their hardened/normal status
        match keys.master_xpriv().child_number {
            ChildNumber::Normal { index } => assert_eq!(index, 0),
            ChildNumber::Hardened { index } => assert_eq!(index, 0),
        }
        match keys.derived_xpriv().child_number {
            ChildNumber::Normal { index } => assert_eq!(index, 0),
            ChildNumber::Hardened { index } => assert_eq!(index, 0),
        }
    }

    #[test]
    #[cfg(feature = "zeroize")]
    fn test_zeroize_idata() {
        fn load_seqkeys() -> IdentityData {
            let master = Xpriv::new_master(Network::Regtest, &[2u8; 32]).unwrap();
            let keys = SequencerKeys::new(&master).unwrap();
            let seq_sk = Buf32::from(keys.derived_xpriv().private_key.secret_bytes());
            let seq_pk = keys.derived_xpub().to_x_only_pub().serialize();
            let ik = IdentityKey::Sequencer(seq_sk);
            let ident = Identity::Sequencer(Buf32::from(seq_pk));

            IdentityData::new(ident, ik)
        }

        fn is_all_zero<T>(p: *const T) -> bool {
            let len = std::mem::size_of::<T>();
            let buf = unsafe { std::slice::from_raw_parts(p as *const u8, len) };
            buf.iter().all(|v| *v == 0)
        }

        let idata = Arc::new(load_seqkeys());
        let raw_ptr = Arc::as_ptr(&idata);

        // In the beginning should not be zeros
        assert!(!is_all_zero(raw_ptr));

        fn bar(_idata_clone: Arc<IdentityData>) {}

        fn foo(idata: Arc<IdentityData>) {
            bar(idata.clone());
        }

        foo(idata.clone());

        // Drop the last reference, triggering `ZeroizeOnDrop`
        drop(idata);

        // Now check if the memory has been zeroized
        assert!(is_all_zero(raw_ptr), "contents {:?}", raw_ptr);
    }
}
