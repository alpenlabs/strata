use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::l1::{L1HeaderPayload, L1Tx};
use rockbound::schema::{
    ColumnFamilyName, KeyDecoder, KeyEncoder, Result as RockResult, ValueCodec,
};
use rockbound::CodecError;
use rockbound::Schema;

#[derive(Debug)]
pub struct L1BlockSchema;

#[derive(Debug)]
pub struct TxnSchema;

#[derive(Debug)]
pub struct MmrSchema;

// L1BlockSchema and corresponding codecs implementation
impl Schema for L1BlockSchema {
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "l1_blocks";
    type Key = Buf32;
    type Value = L1HeaderPayload;
}

impl KeyEncoder<L1BlockSchema> for Buf32 {
    fn encode_key(&self) -> RockResult<Vec<u8>> {
        Ok(self.0.to_vec())
    }
}
impl KeyDecoder<L1BlockSchema> for Buf32 {
    fn decode_key(data: &[u8]) -> RockResult<Self> {
        if data.len() != 32 {
            return Err(CodecError::InvalidKeyLength {
                expected: 32,
                got: data.len(),
            });
        };
        Ok(borsh::from_slice(data)?)
    }
}

impl ValueCodec<L1BlockSchema> for L1HeaderPayload {
    fn encode_value(&self) -> RockResult<Vec<u8>> {
        Ok(borsh::to_vec(self)?)
    }

    fn decode_value(data: &[u8]) -> RockResult<Self> {
        Ok(borsh::from_slice(data)?)
    }
}

// TxnSchema and corresponding codecs implementation
impl Schema for TxnSchema {
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "l1_txns";
    type Key = Buf32; // TxId
    type Value = L1Tx;
}

impl KeyEncoder<TxnSchema> for Buf32 {
    fn encode_key(&self) -> RockResult<Vec<u8>> {
        Ok(self.0.to_vec())
    }
}

impl KeyDecoder<TxnSchema> for Buf32 {
    fn decode_key(data: &[u8]) -> RockResult<Self> {
        if data.len() != 32 {
            return Err(CodecError::InvalidKeyLength {
                expected: 32,
                got: data.len(),
            });
        };
        Ok(borsh::from_slice(data)?)
    }
}

impl ValueCodec<TxnSchema> for L1Tx {
    fn encode_value(&self) -> RockResult<Vec<u8>> {
        Ok(borsh::to_vec(self)?)
    }

    fn decode_value(data: &[u8]) -> RockResult<Self> {
        Ok(borsh::from_slice(data)?)
    }
}

// Mmr Schema and corresponding codecs implementation
type MmrKey = u32; // TODO: change appropriately
type MmrValue = u32; // TODO: change appropriately

impl Schema for MmrSchema {
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "mmr_headers";
    type Key = MmrKey;
    type Value = MmrValue;
}

impl KeyEncoder<MmrSchema> for MmrKey {
    fn encode_key(&self) -> RockResult<Vec<u8>> {
        todo!()
    }
}

impl KeyDecoder<MmrSchema> for MmrKey {
    fn decode_key(data: &[u8]) -> RockResult<Self> {
        todo!()
    }
}

impl ValueCodec<MmrSchema> for MmrValue {
    fn encode_value(&self) -> RockResult<Vec<u8>> {
        todo!()
    }

    fn decode_value(data: &[u8]) -> RockResult<Self> {
        todo!()
    }
}
