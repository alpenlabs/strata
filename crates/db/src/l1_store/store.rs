use std::{marker::PhantomData, path::Path};

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::l1::{L1HeaderPayload, L1Tx};
use rockbound::{Schema, DB};
use rocksdb::Options;

use super::{
    schemas::{L1BlockSchema, MmrSchema, TxnSchema},
    traits::L1StoreTrait,
};

const DB_NAME: &str = "l1_store";

pub struct L1Store {
    db: DB,
    block_schema: L1BlockSchema,
    txn_schema: TxnSchema,
    mmr_schema: MmrSchema,
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
            L1BlockSchema::COLUMN_FAMILY_NAME,
            TxnSchema::COLUMN_FAMILY_NAME,
            MmrSchema::COLUMN_FAMILY_NAME,
        ];
        let store = Self {
            db: DB::open(path, DB_NAME, column_families, &db_opts)?,
            block_schema: L1BlockSchema,
            txn_schema: TxnSchema,
            mmr_schema: MmrSchema,
        };
        Ok(store)
    }
}

impl L1StoreTrait for L1Store {
    fn put_header(
        &self,
        header_hash: Buf32,
        header_payload: L1HeaderPayload,
    ) -> anyhow::Result<()> {
        self.db.put::<L1BlockSchema>(&header_hash, &header_payload)
    }

    fn put_transaction(&self, txid: Buf32, tx: alpen_vertex_state::l1::L1Tx) -> anyhow::Result<()> {
        self.db.put::<TxnSchema>(&txid, &tx)
    }

    fn revert(&mut self, nblocks: u32) -> anyhow::Result<()> {
        // Does nothing useful at the moment
        Ok(())
    }

    fn commit(&mut self) -> anyhow::Result<()> {
        // Does nothing useful at the moment because put_* is adding directly to the db
        Ok(())
    }

    fn get_transaction(&self, txid: Buf32) -> Option<L1Tx> {
        self.db.get::<TxnSchema>(&txid).unwrap()
    }

    fn get_header(&self, header_hash: Buf32) -> Option<L1HeaderPayload> {
        self.db.get::<L1BlockSchema>(&header_hash).unwrap()
    }
}
