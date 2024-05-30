//! Defines the tables used by JMT
//! Adapted from sov-sdk

use core::fmt::Debug;
use std::io::Cursor;

use alpen_vertex_db::define_table_without_codec;

use borsh::{BorshDeserialize, BorshSerialize};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use jmt::Version;
use jmt::storage::{NibblePath, Node, NodeKey};
use rockbound::schema::{KeyDecoder, KeyEncoder, ValueCodec};
use rockbound::{CodecError, SeekKeyEncoder};
use rockbound::{SchemaKey, SchemaValue};

define_table_without_codec!(
    /// A table to store mapping from node hash to JMT node
    (KeyHashToKey) [u8; 32] => SchemaKey
);

define_table_without_codec!(
    /// A table to store mapping from (key, version) to JMT value
    (JmtValues) (SchemaKey, Version) => Option<SchemaValue>
);

define_table_without_codec!(
    /// A table to store mapping from NodeKey to Node
    (JmtNodes) NodeKey => Node
);

impl KeyEncoder<KeyHashToKey> for [u8; 32] {
    fn encode_key(&self) -> Result<Vec<u8>, CodecError> {
        borsh::to_vec(self).map_err(Into::into)
    }
}

impl KeyDecoder<KeyHashToKey> for [u8; 32] {
    fn decode_key(data: &[u8]) -> Result<Self, CodecError> {
        BorshDeserialize::deserialize_reader(&mut &data[..]).map_err(Into::into)
    }
}

impl ValueCodec<KeyHashToKey> for SchemaKey {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        borsh::to_vec(self).map_err(Into::into)
    }

    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        BorshDeserialize::deserialize_reader(&mut &data[..]).map_err(Into::into)
    }
}

impl KeyEncoder<JmtNodes> for NodeKey {
    fn encode_key(&self) -> Result<Vec<u8>, CodecError> {
        // 8 bytes for version, 4 each for the num_nibbles and bytes.len() fields, plus 1 byte per byte of nibllepath
        let mut output =
            Vec::with_capacity(8 + 4 + 4 + ((self.nibble_path().num_nibbles() + 1) / 2));
        let version = self.version().to_be_bytes();
        output.extend_from_slice(&version);
        BorshSerialize::serialize(&self.nibble_path(), &mut output)?;
        Ok(output)
    }
}
impl KeyDecoder<JmtNodes> for NodeKey {
    fn decode_key(data: &[u8]) -> Result<Self, CodecError> {
        if data.len() < 8 {
            return Err(CodecError::InvalidKeyLength {
                expected: 9,
                got: data.len(),
            });
        }
        let mut version = [0u8; 8];
        version.copy_from_slice(&data[..8]);
        let version = u64::from_be_bytes(version);
        let nibble_path = NibblePath::deserialize_reader(&mut &data[8..])?;
        Ok(Self::new(version, nibble_path))
    }
}

impl ValueCodec<JmtNodes> for Node {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        borsh::to_vec(self).map_err(CodecError::from)
    }

    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        Ok(Self::deserialize_reader(&mut &data[..])?)
    }
}

impl<T: Debug + PartialEq + AsRef<[u8]>> KeyEncoder<JmtValues> for (T, Version) {
    fn encode_key(&self) -> Result<Vec<u8>, CodecError> {
        let mut out =
            Vec::with_capacity(self.0.as_ref().len() + std::mem::size_of::<Version>() + 8);
        BorshSerialize::serialize(self.0.as_ref(), &mut out).map_err(CodecError::from)?;
        // Write the version in big-endian order so that sorting order is based on the most-significant bytes of the key
        out.write_u64::<BigEndian>(self.1)
            .expect("serialization to vec is infallible");
        Ok(out)
    }
}

impl<T: AsRef<[u8]> + PartialEq + Debug> SeekKeyEncoder<JmtValues> for (T, Version) {
    fn encode_seek_key(&self) -> Result<Vec<u8>, CodecError> {
        <(T, Version) as KeyEncoder<JmtValues>>::encode_key(self)
    }
}

impl KeyDecoder<JmtValues> for (SchemaKey, Version) {
    fn decode_key(data: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = Cursor::new(data);
        let key = Vec::<u8>::deserialize_reader(&mut cursor)?;
        let version = cursor.read_u64::<BigEndian>()?;
        Ok((key, version))
    }
}

impl ValueCodec<JmtValues> for Option<SchemaValue> {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        borsh::to_vec(self).map_err(CodecError::from)
    }

    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        Ok(Self::deserialize_reader(&mut &data[..])?)
    }
}
