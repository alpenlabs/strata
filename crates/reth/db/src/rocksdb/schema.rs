use anyhow::Context;
use bincode::Options;
use reth_primitives::B256;

use alpen_express_rocksdb::define_table_without_codec;

define_table_without_codec!(
    /// store of block witness data. Data stored as serialized bytes for directly serving in rpc
    (BlockWitnessSchema) B256 => Vec<u8>
);

impl rockbound::schema::KeyEncoder<BlockWitnessSchema> for B256 {
    fn encode_key(&self) -> ::std::result::Result<::std::vec::Vec<u8>, rockbound::CodecError> {
        let bincode_options = ::bincode::options()
            .with_fixint_encoding()
            .with_big_endian();

        bincode_options
            .serialize(self)
            .context("Failed to serialize key")
            .map_err(Into::into)
    }
}

impl rockbound::schema::KeyDecoder<BlockWitnessSchema> for B256 {
    fn decode_key(data: &[u8]) -> ::std::result::Result<Self, rockbound::CodecError> {
        let bincode_options = ::bincode::options()
            .with_fixint_encoding()
            .with_big_endian();

        bincode_options
            .deserialize_from(&mut &data[..])
            .context("Failed to deserialize key")
            .map_err(Into::into)
    }
}
// impl rockbound::schema::ValueCodec<BlockWitnessSchema> for ZKVMInput {
//     fn encode_value(&self) -> ::std::result::Result<::std::vec::Vec<u8>, rockbound::CodecError> {
//         let bincode_options = ::bincode::options()
//             .with_fixint_encoding()
//             .with_big_endian();

//         bincode_options
//             .serialize(self)
//             .context("Failed to serialize value")
//             .map_err(Into::into)
//     }

//     fn decode_value(data: &[u8]) -> ::std::result::Result<Self, rockbound::CodecError> {
//         let bincode_options = ::bincode::options()
//             .with_fixint_encoding()
//             .with_big_endian();

//         bincode_options
//             .deserialize_from(&mut &data[..])
//             .context("Failed to deserialize value")
//             .map_err(Into::into)
//     }
// }

impl rockbound::schema::ValueCodec<BlockWitnessSchema> for Vec<u8> {
    fn encode_value(&self) -> ::std::result::Result<::std::vec::Vec<u8>, rockbound::CodecError> {
        Ok(self.to_vec())
    }

    fn decode_value(data: &[u8]) -> ::std::result::Result<Self, rockbound::CodecError> {
        Ok(data.to_vec())
    }
}
