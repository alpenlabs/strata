use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::alloy_primitives::FixedBytes;

// 20-byte buf
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf20(pub FixedBytes<20>);

// 32-byte buf, useful for hashes and schnorr pubkeys
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf32(pub FixedBytes<32>);

// 64-byte buf, useful for schnorr signatures
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Buf64(pub FixedBytes<64>);

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
