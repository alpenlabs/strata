use rockbound::{Schema, SchemaBatch, DB};
use rocksdb::{Options, ReadOptions};
use std::path::Path;

use alpen_vertex_mmr::CompactMmr;
use alpen_vertex_primitives::{
    buf::Buf32,
    l1::{L1Tx, L1TxRef},
};

use crate::{
    errors::DbError,
    traits::{L1BlockManifest, L1DataProvider, L1DataStore},
    DbResult,
};

use super::schemas::{L1BlockSchema, MmrSchema, TxnSchema};

const DB_NAME: &str = "l1_db";

pub struct L1Db {
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

impl L1Db {
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

impl L1DataStore for L1Db {
    fn put_block_data(&self, idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        // Atomically insert into Block table and txns table. First create batch and then write the
        // batch
        // TODO: check order and throw error accordingly
        let mut batch = SchemaBatch::new();
        batch.put::<L1BlockSchema>(&idx, &mf)?;
        batch.put::<TxnSchema>(&mf.block_hash(), &txs)?;
        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn put_mmr_checkpoint(&self, idx: u64, mmr: CompactMmr) -> DbResult<()> {
        // TODO: check order if relevant
        self.db.put::<MmrSchema>(&idx, &mmr)?;
        Ok(())
    }

    fn revert_to_height(&self, idx: u64) -> DbResult<()> {
        // Get latest height, iterate backwards upto the idx, get blockhash and delete txns and
        // blockmanifest data at each iteration
        let iterator = self.db.iter::<L1BlockSchema>()?.into_iter().rev();
        let mut batch = SchemaBatch::new();
        for res in iterator {
            let (height, blk_manifest) = res?.into_tuple();

            if height < idx {
                break;
            }

            // Get corresponding block hash
            let blockhash = blk_manifest.block_hash();
            // Delete txn data
            batch.delete::<TxnSchema>(&blockhash)?;

            // TODO: Delete mmr data. Don't know what the key exactly should be
            // ...

            // Delete Block manifest data
            batch.delete::<L1BlockSchema>(&height)?;
        }
        // Execute the batch
        self.db.write_schemas(batch)?;
        Ok(())
    }
}

// TODO: Data provider should have readonly db instance. L1Db can write to it as well
impl L1DataProvider for L1Db {
    fn get_tx(&self, tx_ref: alpen_vertex_primitives::l1::L1TxRef) -> DbResult<Option<L1Tx>> {
        todo!()
    }

    fn get_chain_tip(&self) -> DbResult<u64> {
        todo!()
    }

    fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        todo!()
    }

    fn get_last_mmr_to(&self, idx: u64) -> DbResult<Option<CompactMmr>> {
        todo!()
    }

    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>> {
        todo!()
    }

    fn get_block_manifest(&self, idx: u64) -> DbResult<L1BlockManifest> {
        todo!()
    }
}
