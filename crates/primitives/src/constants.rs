//! Constants for magic numbers and strings used in the primitives.

use std::sync::LazyLock;

use bitcoin::XOnlyPublicKey;
use secp256k1::hashes::{sha256, Hash};

/// The size (in bytes) of a [`musig2::PartialSignature`].
pub const MUSIG2_PARTIAL_SIG_SIZE: usize = 32;

/// The size (in bytes) of a [`musig2::NonceSeed`].
pub const NONCE_SEED_SIZE: usize = 32;

/// The size (in bytes) of a [`musig2::PubNonce`].
pub const PUB_NONCE_SIZE: usize = 66;

/// The size (in bytes) of a [`musig2::SecNonce`].
pub const SEC_NONCE_SIZE: usize = 64;

/// The size (in bytes) of a Hash (such as [`Txid`](bitcoin::Txid)).
pub const HASH_SIZE: usize = 32;

/// A verifiably unspendable public key, produced by hashing a fixed string to a curve group
/// generator.
///
/// This is related to the technique used in [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs).
///
/// Note that this is _not_ necessarily a uniformly-sampled curve point!
///
/// But this is fine; we only need a generator with no efficiently-computable discrete logarithm
/// relation against the standard generator.
pub const UNSPENDABLE_PUBLIC_KEY_INPUT: &'static [u8; 18] = b"Strata unspendable";
pub static UNSPENDABLE_PUBLIC_KEY: LazyLock<XOnlyPublicKey> = LazyLock::new(|| {
    XOnlyPublicKey::from_slice(sha256::Hash::hash(UNSPENDABLE_PUBLIC_KEY_INPUT).as_byte_array())
        .expect("valid xonly public key")
});

#[cfg(test)]
mod tests {
    use super::UNSPENDABLE_PUBLIC_KEY;

    #[test]
    fn test_unspendable() {
        // Check that construction of the unspendable key succeeds
        let _ = *UNSPENDABLE_PUBLIC_KEY;
    }
}
