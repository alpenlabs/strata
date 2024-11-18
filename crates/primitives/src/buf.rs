use std::{
    fmt,
    io::{self, Read, Write},
    str,
};

use arbitrary::Arbitrary;
use bitcoin::{
    hashes::Hash,
    secp256k1::{schnorr, XOnlyPublicKey},
    BlockHash, Txid,
};
use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::revm_primitives::alloy_primitives::FixedBytes;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use crate::errors::ParseError;

// 20-byte buf
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Buf20(pub FixedBytes<20>);

impl Buf20 {
    pub fn zero() -> Self {
        Self([0; 20].into())
    }
}

impl From<[u8; 20]> for Buf20 {
    fn from(value: [u8; 20]) -> Self {
        Self(FixedBytes::from(value))
    }
}

impl fmt::Debug for Buf20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 40];
        hex::encode_to_slice(self.0, &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })
    }
}

impl fmt::Display for Buf20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 6];
        hex::encode_to_slice(&self.0[..3], &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })?;
        f.write_str("..")?;
        hex::encode_to_slice(&self.0[17..], &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })?;
        Ok(())
    }
}

// 32-byte buf, useful for hashes and schnorr pubkeys
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Deserialize, Serialize)]
pub struct Buf32(pub FixedBytes<32>);

impl Buf32 {
    pub fn zero() -> Self {
        Self([0; 32].into())
    }

    pub fn is_zero(&self) -> bool {
        *self.as_ref() == [0; 32]
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<[u8; 32]> for Buf32 {
    fn from(value: [u8; 32]) -> Self {
        Self(FixedBytes::from(value))
    }
}

impl From<Buf32> for FixedBytes<32> {
    fn from(value: Buf32) -> Self {
        value.0
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
        XOnlyPublicKey::from_slice(&value.0 .0).map_err(|_| ParseError::InvalidPoint(value))
    }
}

impl<'a> TryFrom<&'a [u8]> for Buf32 {
    type Error = &'a [u8];

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() == 32 {
            let mut arr = [0; 32];
            arr.copy_from_slice(value);
            Ok(Self(arr.into()))
        } else {
            Err(value)
        }
    }
}

impl From<XOnlyPublicKey> for Buf32 {
    fn from(value: XOnlyPublicKey) -> Self {
        Self::from(value.serialize())
    }
}

impl AsRef<[u8; 32]> for Buf32 {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Buf32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 64];
        hex::encode_to_slice(self.0, &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })
    }
}

impl fmt::Display for Buf32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 6];
        hex::encode_to_slice(&self.0[..3], &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })?;
        f.write_str("..")?;
        hex::encode_to_slice(&self.0[29..], &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })?;
        Ok(())
    }
}

// 64-byte buf, useful for schnorr signatures
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Buf64(pub FixedBytes<64>);

impl Buf64 {
    pub fn zero() -> Self {
        Self([0; 64].into())
    }
}

impl fmt::Debug for Buf64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 128];
        hex::encode_to_slice(self.0, &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })
    }
}

impl From<[u8; 64]> for Buf64 {
    fn from(value: [u8; 64]) -> Self {
        Self(FixedBytes::from(value))
    }
}

impl AsRef<[u8; 64]> for Buf64 {
    fn as_ref(&self) -> &[u8; 64] {
        &self.0
    }
}

impl From<schnorr::Signature> for Buf64 {
    fn from(value: schnorr::Signature) -> Self {
        value.serialize().into()
    }
}

impl BorshSerialize for Buf20 {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for Buf20 {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut array = [0u8; 20];
        reader.read_exact(&mut array)?;
        Ok(Self(array.into()))
    }
}

impl BorshSerialize for Buf32 {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for Buf32 {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut array = [0u8; 32];
        reader.read_exact(&mut array)?;
        Ok(Self(array.into()))
    }
}

impl BorshSerialize for Buf64 {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for Buf64 {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut array = [0u8; 64];
        reader.read_exact(&mut array)?;
        Ok(Self(array.into()))
    }
}

impl<'a> Arbitrary<'a> for Buf20 {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut array = [0u8; 20];
        u.fill_buffer(&mut array)?;
        Ok(Buf20(array.into()))
    }
}

impl<'a> Arbitrary<'a> for Buf32 {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut array = [0u8; 32];
        u.fill_buffer(&mut array)?;
        Ok(Buf32(array.into()))
    }
}

impl<'a> Arbitrary<'a> for Buf64 {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut array = [0u8; 64];
        u.fill_buffer(&mut array)?;
        Ok(Buf64(array.into()))
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
