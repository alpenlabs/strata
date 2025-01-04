use std::str::FromStr;

use bitcoin::{
    hashes::Hash,
    secp256k1::{schnorr, SecretKey, XOnlyPublicKey},
    BlockHash, Txid, Wtxid,
};
use reth_primitives::revm_primitives::alloy_primitives::hex;
#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

use crate::{errors::ParseError, macros::internal};

/// A 20-byte buffer.
///
/// # Warning
///
/// This type is not zeroized on drop.
/// However, it implements the [`Zeroize`] trait, so you can zeroize it manually.
/// This is useful for secret data that needs to be zeroized after use.
///
/// # Example
///
/// ```
/// # use strata_primitives::prelude::Buf20;
/// use zeroize::Zeroize;
///
/// let mut buf = Buf20::from([1; 20]);
/// buf.zeroize();
///
/// assert_eq!(buf, Buf20::from([0; 20]));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf20(pub [u8; 20]);
internal::impl_buf_common!(Buf20, 20);
internal::impl_buf_serde!(Buf20, 20);

// NOTE: we cannot do `ZeroizeOnDrop` since `Buf20` is `Copy`.
impl Zeroize for Buf20 {
    #[inline]
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// A 32-byte buffer.
///
/// This is useful for hashes, transaction IDs, secret and public keys.
///
/// # Warning
///
/// This type is not zeroized on drop.
/// However, it implements the [`Zeroize`] trait, so you can zeroize it manually.
/// This is useful for secret data that needs to be zeroized after use.
///
/// # Example
///
/// ```
/// # use strata_primitives::prelude::Buf32;
/// use zeroize::Zeroize;
///
/// let mut buf = Buf32::from([1; 32]);
/// buf.zeroize();
///
/// assert_eq!(buf, Buf32::from([0; 32]));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf32(pub [u8; 32]);
internal::impl_buf_common!(Buf32, 32);
internal::impl_buf_serde!(Buf32, 32);

impl FromStr for Buf32 {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode_to_array(s).map(Self::new)
    }
}

impl From<BlockHash> for Buf32 {
    fn from(value: BlockHash) -> Self {
        (*value.as_raw_hash().as_byte_array()).into()
    }
}

impl From<Txid> for Buf32 {
    fn from(value: Txid) -> Self {
        let bytes: [u8; 32] = *value.as_raw_hash().as_byte_array();
        bytes.into()
    }
}

impl From<Buf32> for Txid {
    fn from(value: Buf32) -> Self {
        let mut bytes: [u8; 32] = [0; 32];
        bytes.copy_from_slice(value.0.as_slice());
        Txid::from_byte_array(bytes)
    }
}

impl From<Wtxid> for Buf32 {
    fn from(value: Wtxid) -> Self {
        let bytes: [u8; 32] = *value.as_raw_hash().as_byte_array();
        bytes.into()
    }
}

impl From<Buf32> for Wtxid {
    fn from(value: Buf32) -> Self {
        let mut bytes: [u8; 32] = [0; 32];
        bytes.copy_from_slice(value.0.as_slice());
        Wtxid::from_byte_array(bytes)
    }
}

impl From<SecretKey> for Buf32 {
    fn from(value: SecretKey) -> Self {
        let bytes: [u8; 32] = value.secret_bytes();
        bytes.into()
    }
}

impl From<Buf32> for SecretKey {
    fn from(value: Buf32) -> Self {
        SecretKey::from_slice(value.0.as_slice()).expect("could not convert Buf32 into SecretKey")
    }
}

impl TryFrom<Buf32> for XOnlyPublicKey {
    type Error = ParseError;

    fn try_from(value: Buf32) -> Result<Self, Self::Error> {
        XOnlyPublicKey::from_slice(&value.0).map_err(|_| ParseError::InvalidPoint(value))
    }
}

impl From<XOnlyPublicKey> for Buf32 {
    fn from(value: XOnlyPublicKey) -> Self {
        Self::from(value.serialize())
    }
}

// NOTE: we cannot do `ZeroizeOnDrop` since `Buf32` is `Copy`.
#[cfg(feature = "zeroize")]
impl Zeroize for Buf32 {
    #[inline]
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

/// A 64-byte buffer.
///
/// This is useful for schnorr signatures.
///
/// # Warning
///
/// This type is not zeroized on drop.
/// However, it implements the [`Zeroize`] trait, so you can zeroize it manually.
/// This is useful for secret data that needs to be zeroized after use.
///
/// # Example
///
/// ```
/// # use strata_primitives::prelude::Buf64;
/// use zeroize::Zeroize;
///
/// let mut buf = Buf64::from([1; 64]);
/// buf.zeroize();
///
/// assert_eq!(buf, Buf64::from([0; 64]));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf64(pub [u8; 64]);
internal::impl_buf_common!(Buf64, 64);
internal::impl_buf_serde!(Buf64, 64);

impl From<schnorr::Signature> for Buf64 {
    fn from(value: schnorr::Signature) -> Self {
        value.serialize().into()
    }
}

// NOTE: we cannot do `ZeroizeOnDrop` since `Buf64` is `Copy`.
#[cfg(feature = "zeroize")]
impl Zeroize for Buf64 {
    #[inline]
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buf32_deserialization() {
        // without 0x
        assert_eq!(
            Buf32::from([0; 32]),
            serde_json::from_str(
                "\"0000000000000000000000000000000000000000000000000000000000000000\"",
            )
            .unwrap()
        );

        // with 0x
        assert_eq!(
            Buf32::from([1; 32]),
            serde_json::from_str(
                "\"0x0101010101010101010101010101010101010101010101010101010101010101\"",
            )
            .unwrap()
        );

        // correct byte order
        assert_eq!(
            Buf32::from([
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 170u8
            ]),
            serde_json::from_str(
                "\"0x01010101010101010101010101010101010101010101010101010101010101aa\"",
            )
            .unwrap()
        );
    }

    #[test]
    fn test_buf32_serialization() {
        assert_eq!(
            serde_json::to_string(&Buf32::from([0; 32])).unwrap(),
            String::from("\"0x0000000000000000000000000000000000000000000000000000000000000000\"")
        );

        assert_eq!(
            serde_json::to_string(&Buf32::from([
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
                1, 1, 1, 170u8
            ]))
            .unwrap(),
            String::from("\"0x01010101010101010101010101010101010101010101010101010101010101aa\"")
        );
    }

    #[test]
    #[cfg(feature = "zeroize")]
    fn test_zeroize() {
        let mut buf20 = Buf20::from([1; 20]);
        let mut buf32 = Buf32::from([1; 32]);
        let mut buf64 = Buf64::from([1; 64]);
        buf20.zeroize();
        buf32.zeroize();
        buf64.zeroize();
        assert_eq!(buf20, Buf20::from([0; 20]));
        assert_eq!(buf32, Buf32::from([0; 32]));
        assert_eq!(buf64, Buf64::from([0; 64]));
    }
}
