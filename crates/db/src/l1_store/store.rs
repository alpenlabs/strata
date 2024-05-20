use std::path::Path;

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::l1::{L1HeaderPayload, L1Tx};
use rockbound::{
    schema::{ColumnFamilyName, KeyDecoder, KeyEncoder, Result as RockResult, ValueCodec},
    CodecError, Schema, DB,
};
use rocksdb::Options;

use super::traits::L1StoreTrait;

const DB_NAME: &str = "l1_store";

pub struct L1Store {
    db: DB,
    block_store: L1BlockStore,
    txn_store: TxnStore,
    // mmr_checkpoints: ??
}

fn get_db_opts() -> Options {
    // TODO: add other options as appropriate.
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);
    db_opts
}

impl L1Store {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let db_opts = get_db_opts();
        let column_families = vec![
            L1BlockStore::COLUMN_FAMILY_NAME,
            // TODO: add others as well
        ];
        let store = Self {
            db: DB::open(path, DB_NAME, column_families, &db_opts)?,
            block_store: L1BlockStore,
            txn_store: TxnStore,
        };
        Ok(store)
    }
}

#[derive(Debug)]
struct L1BlockStore;

#[derive(Debug)]
struct TxnStore;

impl Schema for L1BlockStore {
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "l1_blocks";
    type Key = Buf32;
    type Value = L1HeaderPayload;
}

impl KeyEncoder<L1BlockStore> for Buf32 {
    fn encode_key(&self) -> RockResult<Vec<u8>> {
        Ok(self.0.to_vec())
    }
}
impl KeyDecoder<L1BlockStore> for Buf32 {
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

impl ValueCodec<L1BlockStore> for L1HeaderPayload {
    fn encode_value(&self) -> RockResult<Vec<u8>> {
        Ok(borsh::to_vec(self)?)
    }

    fn decode_value(data: &[u8]) -> RockResult<Self> {
        Ok(borsh::from_slice(data)?)
    }
}

impl Schema for TxnStore {
    const COLUMN_FAMILY_NAME: ColumnFamilyName = "l1_txns";
    type Key = Buf32; // TxId
    type Value = L1Tx;
}

impl KeyEncoder<TxnStore> for Buf32 {
    fn encode_key(&self) -> RockResult<Vec<u8>> {
        Ok(self.0.to_vec())
    }
}

impl KeyDecoder<TxnStore> for Buf32 {
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

impl ValueCodec<TxnStore> for L1Tx {
    fn encode_value(&self) -> RockResult<Vec<u8>> {
        Ok(borsh::to_vec(self)?)
    }

    fn decode_value(data: &[u8]) -> RockResult<Self> {
        Ok(borsh::from_slice(data)?)
    }
}

impl L1StoreTrait for L1Store {
    fn put_header(
        &self,
        header_hash: alpen_vertex_primitives::prelude::Buf32,
        header_payload: alpen_vertex_state::l1::L1HeaderPayload,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn put_transaction(
        &self,
        txid: alpen_vertex_primitives::prelude::Buf32,
        tx: alpen_vertex_state::l1::L1Tx,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn revert(&mut self, nblocks: u32) -> anyhow::Result<()> {
        Ok(())
    }

    fn commit(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_transaction(
        &self,
        txid: alpen_vertex_primitives::prelude::Buf32,
    ) -> Option<alpen_vertex_state::l1::L1Tx> {
        None
    }

    fn get_header(
        &self,
        header_hash: alpen_vertex_primitives::prelude::Buf32,
    ) -> Option<alpen_vertex_state::l1::L1HeaderPayload> {
        None
    }
}
