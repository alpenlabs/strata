use std::sync::Arc;

use rockbound::{schema::KeyEncoder, SchemaBatch, DB};
use rocksdb::ReadOptions;
use tracing::*;

use alpen_vertex_mmr::CompactMmr;
use alpen_vertex_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1Tx, L1TxRef},
};

use super::schemas::{L1BlockSchema, MmrSchema, TxnSchema};
use crate::errors::*;
use crate::{
    traits::{L1DataProvider, L1DataStore},
};

pub struct L1Db {
    db: Arc<DB>,
}

impl L1Db {
    // NOTE: db is expected to open all the column families defined in STORE_COLUMN_FAMILIES.
    // FIXME: Make it better/generic.
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    pub fn get_latest_block_number(&self) -> DbResult<Option<u64>> {
        let mut iterator = self.db.iter::<L1BlockSchema>()?;
        iterator.seek_to_last();
        let mut rev_iterator = iterator.rev();
        if let Some(res) = rev_iterator.next() {
            let (tip, _) = res?.into_tuple();
            Ok(Some(tip))
        } else {
            Ok(None)
        }
    }
}

impl L1DataStore for L1Db {
    fn put_block_data(&self, idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        // If there is latest block then expect the idx to be 1 greater than the block number, else
        // allow arbitrary block number to be inserted
        match self.get_latest_block_number()? {
            Some(num) if num + 1 != idx => {
                return Err(DbError::OooInsert("l1_store", idx));
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
        // NOTE: mmr idx should correspond to the latest block number. This means block data
        // corresponding to the idx(block_number) is to be inserted before the mmr
        match self.get_latest_block_number()? {
            Some(num) if num != idx => {
                return Err(DbError::OooInsert("l1_store", idx));
            }
            _ => {}
        }
        self.db.put::<MmrSchema>(&idx, &mmr)?;
        Ok(())
    }

    fn revert_to_height(&self, idx: u64) -> DbResult<()> {
        // Get latest height, iterate backwards upto the idx, get blockhash and delete txns and
        // blockmanifest data at each iteration
        let last_block_num = self.get_latest_block_number()?.unwrap_or(0);
        if idx > last_block_num {
            return Err(DbError::Other(
                "Invalid block number to revert to".to_string(),
            ));
        }

        let mut batch = SchemaBatch::new();
        for height in ((idx + 1)..=last_block_num).rev() {
            let blk_manifest = self
                .db
                .get::<L1BlockSchema>(&height)?
                .expect("Expected block not found");

            // Get corresponding block hash
            let blockhash = blk_manifest.block_hash();

            // Delete txn data
            batch.delete::<TxnSchema>(&blockhash)?;

            // Delete MMR data
            batch.delete::<MmrSchema>(&height)?;

            // Delete Block manifest data
            batch.delete::<L1BlockSchema>(&height)?;
        }

        // Execute the batch
        self.db.write_schemas(batch)?;
        Ok(())
    }
}

// Note: Ideally Data Provider should ensure it has only read-only db access,
// this isn't really doable since we're usually opening the database here in
// conjunction with a store instance so we just have to be good about ourselves.
//
// TODO add a test that ensures all the functions still behave as expected when
// opened in read-only mode
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
        self.get_latest_block_number()
            .map(|x| x.unwrap_or_default())
    }

    fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        // TODO eventually change how this is stored so we keep a list of the tx
        // indexes with the smaller manifest so we don't have to load all the
        // interesting transactions twice if we want to look at all of them

        let Some(mf) = self.db.get::<L1BlockSchema>(&idx)? else {
            return Ok(None);
        };

        let Some(txs) = self.db.get::<TxnSchema>(&mf.block_hash())? else {
            warn!(%idx, "missing L1 block body");
            return Err(DbError::MissingL1BlockBody(idx));
        };

        let txs_refs = txs
            .into_iter()
            .map(|tx| L1TxRef::from((idx, tx.proof().position())))
            .collect::<Vec<L1TxRef>>();

        Ok(Some(txs_refs))
    }

    fn get_last_mmr_to(&self, idx: u64) -> DbResult<Option<CompactMmr>> {
        Ok(self.db.get::<MmrSchema>(&idx)?)
    }

    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>> {
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(KeyEncoder::<L1BlockSchema>::encode_key(&start_idx)?);
        options.set_iterate_upper_bound(KeyEncoder::<L1BlockSchema>::encode_key(&end_idx)?);

        let res = self
            .db
            .iter_with_opts::<L1BlockSchema>(options)?
            .map(|item_result| item_result.map(|item| item.into_tuple().1.block_hash()))
            .collect::<Result<Vec<Buf32>, anyhow::Error>>()?;

        Ok(res)
    }

    fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        Ok(self.db.get::<L1BlockSchema>(&idx)?)
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use rand::Rng;
    use tempfile::TempDir;

    use crate::l1::utils::get_db_for_l1_store;

    use super::*;

    struct ArbitraryGenerator {
        buffer: Vec<u8>,
    }

    impl ArbitraryGenerator {
        fn new() -> Self {
            let mut rng = rand::thread_rng();
            // NOTE: 128 should be enough for testing purposes. Change to 256 as needed
            let buffer: Vec<u8> = (0..128).map(|_| rng.gen()).collect();
            ArbitraryGenerator { buffer }
        }

        fn generate<'a, T: Arbitrary<'a> + Clone>(&'a self) -> T {
            let mut u = Unstructured::new(&self.buffer);
            T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
        }
    }

    fn setup_db() -> L1Db {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = get_db_for_l1_store(&temp_dir.into_path()).unwrap();
        L1Db::new(db)
    }

    fn insert_block_data(idx: u64, db: &L1Db) -> (L1BlockManifest, Vec<L1Tx>, CompactMmr) {
        let arb = ArbitraryGenerator::new();

        // TODO maybe tweak this to make it a bit more realistic?
        let mf: L1BlockManifest = arb.generate();
        let txs: Vec<L1Tx> = (0..10)
            .map(|i| {
                let proof = L1TxProof::new(i, arb.generate());
                L1Tx::new(proof, arb.generate())
            })
            .collect();
        let mmr: CompactMmr = arb.generate();

        // Insert block data
        let res = db.put_block_data(idx, mf.clone(), txs.clone());
        assert!(res.is_ok());

        // Insert mmr data
        db.put_mmr_checkpoint(idx, mmr.clone()).unwrap();
        (mf, txs, mmr)
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
            let txs: Vec<L1Tx> = (0..10)
                .map(|_| ArbitraryGenerator::new().generate())
                .collect();
            let res = db.put_block_data(
                invalid_idx,
                ArbitraryGenerator::new().generate::<L1BlockManifest>(),
                txs,
            );
            assert!(res.is_err(), "Should fail to insert to db");
        }

        let valid_idx = idx + 1;
        let txs: Vec<L1Tx> = (0..10)
            .map(|_| ArbitraryGenerator::new().generate())
            .collect();
        let res = db.put_block_data(valid_idx, ArbitraryGenerator::new().generate(), txs);
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

        // Check that some txns and mmrs exists upto this height
        for h in 1..=3 {
            let txn_data = db.get_tx((h, 0).into()).unwrap();
            assert!(txn_data.is_some());
            let mmr_data = db.get_last_mmr_to(h).unwrap();
            assert!(mmr_data.is_some());
        }

        // Check that no txn/mmr exists above the revert height
        let txn_data = db.get_tx((4, 0).into()).unwrap();
        assert!(txn_data.is_none());
        let mmr_data = db.get_last_mmr_to(4).unwrap();
        assert!(mmr_data.is_none());
    }

    #[test]
    fn test_put_mmr_checkpoint_invalid() {
        let db = setup_db();
        let _ = insert_block_data(1, &db);
        let mmr: CompactMmr = ArbitraryGenerator::new().generate();
        let invalid_idxs = vec![0, 2, 4, 5, 6, 100, 1000]; // any idx except 1
        for idx in invalid_idxs {
            let res = db.put_mmr_checkpoint(idx, mmr.clone());
            assert!(res.is_err());
        }
    }

    #[test]
    fn test_put_mmr_checkpoint_valid() {
        let db = setup_db();
        let _ = insert_block_data(1, &db);
        let mmr: CompactMmr = ArbitraryGenerator::new().generate();
        let res = db.put_mmr_checkpoint(1, mmr);
        assert!(res.is_ok());
    }

    // TEST PROVIDER METHODS

    #[test]
    fn test_get_block_data() {
        let mut db = setup_db();
        let idx = 1;

        // insert
        let (mf, txs, _) = insert_block_data(idx, &mut db);

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
        let (_, txns, _) = insert_block_data(idx, &db);
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

        let (mf1, _, _) = insert_block_data(1, &db);
        let (mf2, _, _) = insert_block_data(2, &db);
        let (mf3, _, _) = insert_block_data(3, &db);

        let range = db.get_blockid_range(1, 4).unwrap();
        assert_eq!(range.len(), 3);
        for (exp, obt) in vec![mf1, mf2, mf3].iter().zip(range) {
            assert_eq!(exp.block_hash(), obt);
        }
    }

    #[test]
    fn test_get_last_mmr_to() {
        let db = setup_db();

        let inexistent_idx = 3;
        let mmr = db.get_last_mmr_to(inexistent_idx).unwrap();
        assert!(mmr.is_none());
        let (_, _, mmr) = insert_block_data(1, &db);
        let mmr_res = db.get_last_mmr_to(inexistent_idx).unwrap();
        assert!(mmr_res.is_none());

        // existent mmr
        let observed_mmr = db.get_last_mmr_to(1).unwrap();
        assert_eq!(Some(mmr), observed_mmr);
    }
}
