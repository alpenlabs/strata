use std::fmt;
use std::io::{self, Read, Write};
use std::str;

use arbitrary::Arbitrary;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::alloy_primitives::FixedBytes;
use ssz::{Decode, DecodeError, Encode};

// 20-byte buf
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Buf32(pub FixedBytes<32>);

impl Buf32 {
    pub fn zero() -> Self {
        Self([0; 32].into())
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
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

impl BorshSerialize for Buf20 {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(&bytes)?;
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
        let _ = writer.write(&bytes)?;
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
        let _ = writer.write(&bytes)?;
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

impl Encode for Buf32 {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        32
    }

    fn ssz_bytes_len(&self) -> usize {
        32
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.0 .0);
    }
}

impl Decode for Buf32 {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        32
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() != 32 {
            return Err(DecodeError::InvalidByteLength {
                expected: 32,
                len: bytes.len(),
            });
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(bytes);
        Ok(Buf32(FixedBytes(array)))
    }
}

impl Encode for Buf64 {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        64
    }

    fn ssz_bytes_len(&self) -> usize {
        64
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.0 .0);
    }
}

impl Decode for Buf64 {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        64
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() != 64 {
            return Err(DecodeError::InvalidByteLength {
                expected: 64,
                len: bytes.len(),
            });
        }
        let mut array = [0u8; 64];
        array.copy_from_slice(bytes);
        Ok(Buf64(FixedBytes(array)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssz_buf32() {
        let original = Buf32(FixedBytes([1u8; 32]));

        // Encode
        let encoded: Vec<u8> = original.as_ssz_bytes();
        assert_eq!(encoded.len(), 32);

        // Decode
        let decoded = Buf32::from_ssz_bytes(&encoded).unwrap();
        assert_eq!(original.0 .0, decoded.0 .0);
    }

    #[test]
    fn test_ssz_buf64() {
        let original = Buf64(FixedBytes([5u8; 64]));

        // Encode
        let encoded: Vec<u8> = original.as_ssz_bytes();
        assert_eq!(encoded.len(), 64);

        // Decode
        let decoded = Buf64::from_ssz_bytes(&encoded).unwrap();
        assert_eq!(original.0 .0, decoded.0 .0);
    }
}
