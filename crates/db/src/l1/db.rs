use anyhow::anyhow;
use rockbound::{schema::KeyEncoder, Schema, SchemaBatch, DB};
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
}

fn get_db_opts() -> Options {
    // TODO: add other options as appropriate.
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);
    db_opts
}

impl L1Db {
    // NOTE: It would be ideal to have just db handle as argument in the new method.
    // However, I want to ensure correct column families being set for db handle. The L1Store
    // implementation below uses *Schema column family name. And if db handle is to be passed from
    // outside, there's no way which stops the handle to be initialized with wrong cf names.
    // I tried fiddling with the types but could not succeed, so I've left it as is for now. Can be
    // enhanced later.
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let db_opts = get_db_opts();
        let column_families = vec![
            L1BlockSchema::COLUMN_FAMILY_NAME,
            TxnSchema::COLUMN_FAMILY_NAME,
            MmrSchema::COLUMN_FAMILY_NAME,
        ];
        let store = Self {
            db: DB::open(path, DB_NAME, column_families, &db_opts)?,
        };
        Ok(store)
    }

    pub fn latest_block_number(&self) -> DbResult<Option<u64>> {
        let mut iterator = self.db.iter::<L1BlockSchema>()?;
        iterator.seek_to_last();
        let mut rev_iterator = iterator.rev();
        if let Some(res) = rev_iterator.next() {
            let (tip, _) = res?.into_tuple();
            return Ok(Some(tip));
        } else {
            Ok(None)
        }
    }
}

impl L1DataStore for L1Db {
    fn put_block_data(&self, idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        // If there is latest block then expect the idx to be 1 greater than the block number, else
        // allow arbitrary block number to be inserted
        match self.latest_block_number()? {
            Some(num) if num + 1 != idx => {
                return Err(DbError::OooInsert("Block store", idx));
            }
            _ => {}
        }
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
        let mut iterator = self.db.iter::<L1BlockSchema>()?;
        iterator.seek_to_last();
        let rev_iterator = iterator.rev();

        let last_block_num = self.latest_block_number()?.unwrap_or(0);
        if idx > last_block_num {
            return Err(DbError::Other(
                "Invalid block number to revert to".to_string(),
            ));
        }

        let mut batch = SchemaBatch::new();
        for res in rev_iterator {
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

// Note: Ideally Data Provider should ensure it has only read-only db access.
impl L1DataProvider for L1Db {
    fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        let (block_height, txindex) = tx_ref.into();
        let tx = self
            .db
            .get::<L1BlockSchema>(&(block_height as u64))
            .and_then(|mf_opt| match mf_opt {
                Some(mf) => {
                    let txs_opt = self.db.get::<TxnSchema>(&mf.block_hash())?;
                    let tx = txs_opt.and_then(|txs| txs.get(txindex as usize).cloned());
                    Ok(tx)
                }
                None => Ok(None),
            });
        Ok(tx?)
    }

    fn get_chain_tip(&self) -> DbResult<u64> {
        self.latest_block_number().map(|x| x.unwrap_or_default())
    }

    fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        let txs = self
            .db
            .get::<L1BlockSchema>(&idx)
            .and_then(|mf_opt| match mf_opt {
                Some(mf) => {
                    let txs_opt = self.db.get::<TxnSchema>(&mf.block_hash())?;
                    Ok(txs_opt.map(|txs| {
                        txs.into_iter()
                            .enumerate()
                            .map(|(i, _)| L1TxRef::from((idx.clone().into(), i as u32)))
                            .collect()
                    }))
                }
                None => Err(anyhow!("Cound not find block txns")),
            });
        Ok(txs?)
    }

    fn get_last_mmr_to(&self, _idx: u64) -> DbResult<Option<CompactMmr>> {
        todo!()
    }

    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>> {
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(KeyEncoder::<L1BlockSchema>::encode_key(&start_idx)?);
        options.set_iterate_upper_bound(KeyEncoder::<L1BlockSchema>::encode_key(&end_idx)?);

        let result = self
            .db
            .iter_with_opts::<L1BlockSchema>(options)?
            .map(|item_result| item_result.map(|item| item.into_tuple().1.block_hash()))
            .collect::<Result<Vec<Buf32>, anyhow::Error>>();
        Ok(result?)
    }

    fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        Ok(self.db.get::<L1BlockSchema>(&idx)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alpen_vertex_primitives::l1::L1TxProof;
    use arbitrary::{Arbitrary, Unstructured};
    use tempfile::TempDir;

    fn generate_arbitrary<'a, T: Arbitrary<'a> + Clone>() -> T {
        let mut u = Unstructured::new(&[1, 2, 3]);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }

    fn setup_db() -> L1Db {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        L1Db::new(temp_dir.path()).expect("failed to create L1Db")
    }

    fn insert_block_data(idx: u64, db: &L1Db) -> (L1BlockManifest, Vec<L1Tx>) {
        let mf: L1BlockManifest = generate_arbitrary();
        // TODO: Use arbitrary generation for txs as well.
        // The txs are unique just by the position of the proof
        let txs: Vec<L1Tx> = (0..10)
            .map(|i| L1Tx::new(L1TxProof::new(i, vec![]), vec![]))
            .collect();

        // Insert block data
        let res = db.put_block_data(idx, mf.clone(), txs.clone());
        assert!(res.is_ok());
        (mf, txs)
    }

    #[test]
    fn test_initialization() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = L1Db::new(temp_dir.path());
        assert!(db.is_ok());
    }

    // TEST STORE METHODS

    #[test]
    fn test_insert_into_empty_db() {
        let db = setup_db();
        let idx = 1;
        insert_block_data(idx, &db);
        drop(db);

        // insert another block with arbitrary id
        let db = setup_db();
        let idx = 200011;
        insert_block_data(idx, &db);
    }

    #[test]
    fn test_insert_into_non_empty_db() {
        let mut db = setup_db();
        let idx = 1000;
        insert_block_data(idx, &mut db); // first insertion

        let invalid_idxs = vec![1, 2, 5000, 1000, 1002, 999]; // basically any id beside idx + 1
        for invalid_idx in invalid_idxs {
            let txs: Vec<L1Tx> = (0..10).map(|_| generate_arbitrary()).collect();
            let res = db.put_block_data(invalid_idx, generate_arbitrary::<L1BlockManifest>(), txs);
            assert!(res.is_err(), "Should fail to insert to db");
        }

        let valid_idx = idx + 1;
        let txs: Vec<L1Tx> = (0..10).map(|_| generate_arbitrary()).collect();
        let res = db.put_block_data(valid_idx, generate_arbitrary(), txs);
        assert!(res.is_ok(), "Should successfully insert to db");
    }

    #[test]
    fn test_revert_to_invalid_height() {
        let db = setup_db();
        // First insert a couple of manifests
        let _ = insert_block_data(1, &db);
        let _ = insert_block_data(2, &db);
        let _ = insert_block_data(3, &db);
        let _ = insert_block_data(4, &db);

        // Try reverting to an invalid height, which should fail
        let invalid_heights = [5, 6, 10];
        for inv_h in invalid_heights {
            let res = db.revert_to_height(inv_h);
            assert!(res.is_err(), "Should fail to revert to height {}", inv_h);
        }
    }

    #[test]
    fn test_revert_to_zero_height() {
        let db = setup_db();
        // First insert a couple of manifests
        let _ = insert_block_data(1, &db);
        let _ = insert_block_data(2, &db);
        let _ = insert_block_data(3, &db);
        let _ = insert_block_data(4, &db);

        let res = db.revert_to_height(0);
        assert!(res.is_ok(), "Should succeed to revert to height 0");
    }

    #[test]
    fn test_revert_to_non_zero_height() {
        let db = setup_db();
        // First insert a couple of manifests
        let _ = insert_block_data(1, &db);
        let _ = insert_block_data(2, &db);
        let _ = insert_block_data(3, &db);
        let _ = insert_block_data(4, &db);

        let res = db.revert_to_height(3);
        assert!(res.is_ok(), "Should succeed to revert to non-zero height");
    }

    // TEST PROVIDER METHODS

    #[test]
    fn test_get_block_data() {
        let mut db = setup_db();
        let idx = 1;

        // insert
        let (mf, txs) = insert_block_data(idx, &mut db);

        // fetch non existent block
        let non_idx = 200;
        let observed_mf = db
            .get_block_manifest(non_idx)
            .expect("Could not fetch from db");
        assert_eq!(observed_mf, None);

        // fetch and check, existent block
        let observed_mf = db.get_block_manifest(idx).expect("Could not fetch from db");
        assert_eq!(observed_mf, Some(mf));

        // Fetch txs
        for (i, tx) in txs.iter().enumerate() {
            let tx_from_db = db
                .get_tx((idx, i as u32).into())
                .expect("Can't fetch from db")
                .unwrap();
            assert_eq!(*tx, tx_from_db, "Txns should match at index {}", i);
        }
    }

    #[test]
    fn test_get_tx() {
        let db = setup_db();
        let idx = 1; // block number
                     // Insert a block
        let (_, txns) = insert_block_data(idx, &db);
        let txidx: u32 = 3; // some tx index
        assert!(txns.len() > txidx as usize);
        let tx_ref: L1TxRef = (1, txidx).into();
        let tx = db.get_tx(tx_ref);
        assert!(tx.as_ref().unwrap().is_some());
        let tx = tx.unwrap().unwrap().clone();
        assert_eq!(
            tx,
            *txns.get(txidx as usize).unwrap(),
            "Should fetch correct transaction"
        );
        // Check txn at different index. It should not match
        assert_ne!(
            tx,
            *txns.get(txidx as usize + 1).unwrap(),
            "Txn at different index should not match"
        );
    }

    #[test]
    fn test_get_chain_tip() {
        let db = setup_db();
        assert_eq!(
            db.get_chain_tip().unwrap(),
            0,
            "Chain tip of empty db should be zero"
        );

        // Insert some block data
        insert_block_data(1, &db);
        assert_eq!(db.get_chain_tip().unwrap(), 1);
        insert_block_data(2, &db);
        assert_eq!(db.get_chain_tip().unwrap(), 2);
    }

    #[test]
    fn test_get_block_txs() {
        let db = setup_db();

        insert_block_data(1, &db);
        insert_block_data(2, &db);
        insert_block_data(3, &db);

        let block_txs = db.get_block_txs(2).unwrap().unwrap();
        let expected: Vec<_> = (0..10).map(|i| (2, i).into()).collect(); // 10 because insert_block_data inserts 10 txs
        assert_eq!(block_txs, expected);
    }

    #[test]
    fn test_get_blockid_invalid_range() {
        let db = setup_db();

        let _ = insert_block_data(1, &db);
        let _ = insert_block_data(2, &db);
        let _ = insert_block_data(3, &db);

        let range = db.get_blockid_range(3, 1).unwrap();
        assert_eq!(range.len(), 0);
    }

    #[test]
    fn test_get_blockid_range() {
        let db = setup_db();

        let (mf1, _) = insert_block_data(1, &db);
        let (mf2, _) = insert_block_data(2, &db);
        let (mf3, _) = insert_block_data(3, &db);

        let range = db.get_blockid_range(1, 4).unwrap();
        assert_eq!(range.len(), 3);
        for (exp, obt) in vec![mf1, mf2, mf3].iter().zip(range) {
            assert_eq!(exp.block_hash(), obt);
        }
    }
}
