//! Constants for magic numbers and strings used in the primitives.

use std::sync::LazyLock;

use bitcoin::{
    bip32::{ChildNumber, DerivationPath},
    XOnlyPublicKey,
};
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

/// Strata base index for keys.
///
/// These should be _hardened_ [`ChildNumber`].
///
/// # Implementation Details
///
/// The base index is set to 20,000 to ensure that it does not conflict with
/// [BIP-43](https://github.com/bitcoin/bips/blob/master/bip-0043.mediawiki)
/// reserved ranges.
pub const STRATA_BASE_IDX: u32 = 20_000;

/// Strata sequencer index for keys.
///
/// NOTE: These should be _hardened_.
pub const STRATA_SEQUENCER_IDX: u32 = 10;

/// Strata operator index for keys.
///
/// These should be _hardened_ [`ChildNumber`].
pub const STRATA_OPERATOR_IDX: u32 = 20;

/// Strata message index for the operator message key.
///
/// These should be _normal_ [`ChildNumber`].
pub const STRATA_OPERATOR_MESSAGE_IDX: u32 = 100;

/// Strata Wallet index for the operator wallet key.
///
/// NOTE: These should be _normal_.
pub const STRATA_OPERATOR_WALLET_IDX: u32 = 101;

/// Strata [`DerivationPath`] for sequencer's key.
///
/// This corresponds to the path: `m/20000'/10'`.
pub static STRATA_SEQUENCER_DERIVATION_PATH: LazyLock<DerivationPath> = LazyLock::new(|| {
    DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(STRATA_BASE_IDX).expect("valid hardened child number"),
        ChildNumber::from_hardened_idx(STRATA_SEQUENCER_IDX).expect("valid hardened child number"),
    ])
});

/// Strata base [`DerivationPath`] for operator's message key.
///
/// This corresponds to the path: `m/20000'/20'`.
pub static STRATA_OPERATOR_BASE_DERIVATION_PATH: LazyLock<DerivationPath> = LazyLock::new(|| {
    DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(STRATA_BASE_IDX).expect("valid hardened child number"),
        ChildNumber::from_hardened_idx(STRATA_OPERATOR_IDX).expect("valid hardened child number"),
    ])
});

/// Strata [`DerivationPath`] for operator's key.
///
/// This corresponds to the path: `m/20000'/20'/101`.
pub static STRATA_OP_MESSAGE_DERIVATION_PATH: LazyLock<DerivationPath> = LazyLock::new(|| {
    DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(STRATA_BASE_IDX).expect("valid hardened child number"),
        ChildNumber::from_hardened_idx(STRATA_OPERATOR_IDX).expect("valid hardened child number"),
        ChildNumber::from_normal_idx(STRATA_OPERATOR_MESSAGE_IDX)
            .expect("valid hardened child number"),
    ])
});
/// Strata [`DerivationPath`] for operator's wallet key.
///
/// This corresponds to the path: `m/20000'/20'/101`.
pub static STRATA_OP_WALLET_DERIVATION_PATH: LazyLock<DerivationPath> = LazyLock::new(|| {
    DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(STRATA_BASE_IDX).expect("valid hardened child number"),
        ChildNumber::from_hardened_idx(STRATA_OPERATOR_IDX).expect("valid hardened child number"),
        ChildNumber::from_normal_idx(STRATA_OPERATOR_WALLET_IDX)
            .expect("valid hardened child number"),
    ])
});
/// A verifiably unspendable public key, produced by hashing a fixed string to a curve group
/// generator.
///
/// This is related to the technique used in [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs).
///
/// Note that this is _not_ necessarily a uniformly-sampled curve point!
///
/// But this is fine; we only need a generator with no efficiently-computable discrete logarithm
/// relation against the standard generator.
pub const UNSPENDABLE_PUBLIC_KEY_INPUT: &[u8] = b"Strata unspendable";
pub static UNSPENDABLE_PUBLIC_KEY: LazyLock<XOnlyPublicKey> = LazyLock::new(|| {
    XOnlyPublicKey::from_slice(sha256::Hash::hash(UNSPENDABLE_PUBLIC_KEY_INPUT).as_byte_array())
        .expect("valid xonly public key")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequencer_path() {
        // Check that construction of the sequencer derivation path succeeds
        let _ = *STRATA_SEQUENCER_DERIVATION_PATH;
    }

    #[test]
    fn test_operator_base_path() {
        // Check that construction of the operator base derivation path succeeds
        let _ = *STRATA_OPERATOR_BASE_DERIVATION_PATH;
    }

    #[test]
    fn test_operator_message_path() {
        // Check that construction of the operator message derivation path succeeds
        let _ = *STRATA_OP_MESSAGE_DERIVATION_PATH;
    }

    #[test]
    fn test_operator_wallet_path() {
        // Check that construction of the operator wallet derivation path succeeds
        let _ = *STRATA_OP_WALLET_DERIVATION_PATH;
    }

    #[test]
    fn test_unspendable() {
        // Check that construction of the unspendable key succeeds
        let _ = *UNSPENDABLE_PUBLIC_KEY;
    }
}
