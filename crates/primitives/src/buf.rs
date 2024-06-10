use std::fmt;
use std::str;

use arbitrary::Arbitrary;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::alloy_primitives::FixedBytes;

// 20-byte buf
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

// 32-byte buf, useful for hashes and schnorr pubkeys
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

impl From<BlockHash> for Buf32 {
    fn from(value: BlockHash) -> Self {
        (*value.as_raw_hash().as_byte_array()).into()
    }
}

impl fmt::Debug for Buf32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0; 64];
        hex::encode_to_slice(self.0, &mut buf).expect("buf: enc hex");
        f.write_str(unsafe { str::from_utf8_unchecked(&buf) })
    }
}

// 64-byte buf, useful for schnorr signatures
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf64(pub FixedBytes<64>);

impl Buf64 {
    pub fn zero() -> Self {
        Self([0; 64].into())
    }
}

impl From<[u8; 64]> for Buf64 {
    fn from(value: [u8; 64]) -> Self {
        Self(FixedBytes::from(value))
    }
}

impl BorshSerialize for Buf32 {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(&bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for Buf32 {
    fn deserialize_reader<R: std::io::prelude::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut array = [0u8; 32];
        reader.read_exact(&mut array)?;
        Ok(Self(array.into()))
    }
}

impl BorshSerialize for Buf64 {
    fn serialize<W: std::io::prelude::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let bytes = self.0.as_ref();
        let _ = writer.write(&bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for Buf64 {
    fn deserialize_reader<R: std::io::prelude::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut array = [0u8; 64];
        reader.read_exact(&mut array)?;
        Ok(Self(array.into()))
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
