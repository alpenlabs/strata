//! Key types used in the Strata library.
//!
//! [`Zeroize`] and [`Zeroize`] on drop should always ensure that keys are zeroized.

use std::ops::Deref;

use bitcoin::{bip32::Xpriv, key::Keypair};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A zeroizable on [`Drop`] wrapper around [`Xpriv`].
#[cfg(feature = "zeroize")]
#[derive(Clone, PartialEq, Eq)]
pub struct ZeroizableXpriv(Xpriv);

impl ZeroizableXpriv {
    /// Create a new [`ZeroizableXpriv`] from an [`Xpriv`].
    ///
    /// This should take ownership of the `xpriv` since it is zeroized on drop.
    pub fn new(xpriv: Xpriv) -> Self {
        Self(xpriv)
    }
}

impl From<Xpriv> for ZeroizableXpriv {
    fn from(xpriv: Xpriv) -> Self {
        Self::new(xpriv)
    }
}

impl Deref for ZeroizableXpriv {
    type Target = Xpriv;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Manual Drop implementation to zeroize keys on drop.
impl Drop for ZeroizableXpriv {
    fn drop(&mut self) {
        #[cfg(feature = "zeroize")]
        self.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for ZeroizableXpriv {
    #[inline]
    fn zeroize(&mut self) {
        self.0.private_key.non_secure_erase();
    }
}

#[cfg(feature = "zeroize")]
impl ZeroizeOnDrop for ZeroizableXpriv {}

/// A zeroizable on [`Drop`] wrapper around [`Keypair`].
#[cfg(feature = "zeroize")]
#[derive(Clone, PartialEq, Eq)]
pub struct ZeroizableKeypair(Keypair);

impl ZeroizableKeypair {
    /// Create a new [`ZeroizableKeypair`] from an [`Keypair`].
    ///
    /// This should take ownership of the `Keypair` since it is zeroized on drop.
    pub fn new(keypair: Keypair) -> Self {
        Self(keypair)
    }
}

impl From<Keypair> for ZeroizableKeypair {
    fn from(keypair: Keypair) -> Self {
        Self::new(keypair)
    }
}

impl Deref for ZeroizableKeypair {
    type Target = Keypair;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Manual Drop implementation to zeroize keypair on drop.
impl Drop for ZeroizableKeypair {
    fn drop(&mut self) {
        #[cfg(feature = "zeroize")]
        self.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl Zeroize for ZeroizableKeypair {
    #[inline]
    fn zeroize(&mut self) {
        self.0.non_secure_erase();
    }
}

#[cfg(feature = "zeroize")]
impl ZeroizeOnDrop for ZeroizableKeypair {}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use super::*;

    // What's better than bacon? bacon^24 of course
    // Thix xpriv was generated by the bacon^24 mnemonic
    // Don't use this in production!
    const XPRIV_STR: &str = "tprv8ZgxMBicQKsPeh9dSitM82FU7Fz3ZgPkKmmovAr2aqwauAMVgjcEkZBb2etBtRPZ8XYVm7shxcKwVaDus7T5kauJXVsqAfzM4Tty13rRjAG";

    #[test]
    fn test_deref() {
        let xpriv = XPRIV_STR.parse::<Xpriv>().unwrap();
        let zeroizable_xpriv = ZeroizableXpriv::new(xpriv);

        assert_eq!(*zeroizable_xpriv, xpriv);
    }

    #[test]
    #[cfg(feature = "zeroize")]
    fn test_zeroizable_xpriv() {
        let xpriv = XPRIV_STR.parse::<Xpriv>().unwrap();
        let mut zeroizable_xpriv = ZeroizableXpriv::new(xpriv);

        // Manually zeroize the key
        zeroizable_xpriv.zeroize();

        // Check that the key is zeroized
        // NOTE: SecretKey::non_secure_erase writes `1`s to the memory.
        assert_eq!(zeroizable_xpriv.private_key.secret_bytes(), [1u8; 32]);
    }

    #[test]
    #[cfg(feature = "zeroize")]
    fn test_zeroize_on_drop_xpriv() {
        // Create an atomic flag to track if zeroize was called
        let was_zeroized = Arc::new(AtomicBool::new(false));
        let was_zeroized_clone = Arc::clone(&was_zeroized);

        // Create a wrapper struct that will set a flag when dropped
        struct TestWrapper {
            inner: ZeroizableXpriv,
            flag: Arc<AtomicBool>,
        }

        impl Drop for TestWrapper {
            fn drop(&mut self) {
                // Get the current value before the inner value is dropped
                let bytes = self.inner.private_key.secret_bytes();
                // The inner ZeroizableXpriv will be dropped after this,
                // triggering zeroization
                // NOTE: SecretKey::non_secure_erase writes `1`s to the memory.
                self.flag.store(bytes != [1u8; 32], Ordering::Relaxed);
            }
        }

        // Create and drop our test wrapper
        {
            let xpriv = XPRIV_STR.parse::<Xpriv>().unwrap();
            let _ = TestWrapper {
                inner: ZeroizableXpriv::new(xpriv),
                flag: was_zeroized_clone,
            };
        }

        // Check if zeroization occurred
        assert!(was_zeroized.load(Ordering::Relaxed));
    }
}
