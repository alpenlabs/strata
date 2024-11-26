use std::str::FromStr;

use bitcoin::{
    hashes::Hash,
    secp256k1::{schnorr, SecretKey, XOnlyPublicKey},
    BlockHash, Txid,
};
use reth_primitives::revm_primitives::alloy_primitives::hex;

use crate::{errors::ParseError, macros::internal};

// 20-byte buf
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf20(pub [u8; 20]);
internal::impl_buf_common!(Buf20, 20);
internal::impl_buf_serde!(Buf20, 20);

// 32-byte buf, useful for hashes and schnorr pubkeys

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

// 64-byte buf, useful for schnorr signatures
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf64(pub [u8; 64]);
internal::impl_buf_common!(Buf64, 64);
internal::impl_buf_serde!(Buf64, 64);

impl From<schnorr::Signature> for Buf64 {
    fn from(value: schnorr::Signature) -> Self {
        value.serialize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::Buf32;

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
}
