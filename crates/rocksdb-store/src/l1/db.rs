use std::sync::Arc;

use rockbound::{
    rocksdb::ReadOptions, schema::KeyEncoder, OptimisticTransactionDB, SchemaBatch,
    SchemaDBOperationsExt,
};
use strata_db::{errors::DbError, traits::*, DbResult};
use strata_mmr::CompactMmr;
use strata_primitives::l1::{L1BlockId, L1BlockManifest, L1Tx, L1TxRef};
use tracing::*;

use super::schemas::{L1BlockSchema, MmrSchema, TxnSchema};
use crate::DbOpsConfig;

pub struct L1Db {
    db: Arc<OptimisticTransactionDB>,
    _ops: DbOpsConfig,
}

impl L1Db {
    // NOTE: db is expected to open all the column families defined in STORE_COLUMN_FAMILIES.
    // FIXME: Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, _ops: ops }
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

impl L1Database for L1Db {
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
        batch.put::<TxnSchema>(mf.blkid(), &txs)?;
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
            let blockhash = blk_manifest.blkid();

            // Delete txn data
            batch.delete::<TxnSchema>(blockhash)?;

            // Delete MMR data
            batch.delete::<MmrSchema>(&height)?;

            // Delete Block manifest data
            batch.delete::<L1BlockSchema>(&height)?;
        }

        // Execute the batch
        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        let (block_height, txindex) = tx_ref.into();
        let tx = self
            .db
            .get::<L1BlockSchema>(&block_height)
            .and_then(|mf_opt| match mf_opt {
                Some(mf) => {
                    let txs_opt = self.db.get::<TxnSchema>(mf.blkid())?;
                    // we only save subset of transaction in a block, while the txindex refers to
                    // original position in txblock.
                    // TODO: txs should be hashmap with original index
                    let tx = txs_opt.and_then(|txs| {
                        txs.iter()
                            .find(|tx| tx.proof().position() == txindex)
                            .cloned()
                    });
                    Ok(tx)
                }
                None => Ok(None),
            });
        Ok(tx?)
    }

    fn get_chain_tip(&self) -> DbResult<Option<u64>> {
        self.get_latest_block_number()
    }

    fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>> {
        // TODO eventually change how this is stored so we keep a list of the tx
        // indexes with the smaller manifest so we don't have to load all the
        // relevant transactions twice if we want to look at all of them

        let Some(mf) = self.db.get::<L1BlockSchema>(&idx)? else {
            return Ok(None);
        };

        let Some(txs) = self.db.get::<TxnSchema>(mf.blkid())? else {
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

    // TODO: This should not exist in database level and should be handled by downstream manager
    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<L1BlockId>> {
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(
            KeyEncoder::<L1BlockSchema>::encode_key(&start_idx)
                .map_err(|err| DbError::CodecError(err.to_string()))?,
        );
        options.set_iterate_upper_bound(
            KeyEncoder::<L1BlockSchema>::encode_key(&end_idx)
                .map_err(|err| DbError::CodecError(err.to_string()))?,
        );

        let res = self
            .db
            .iter_with_opts::<L1BlockSchema>(options)?
            .map(|item_result| item_result.map(|item| *item.into_tuple().1.blkid()))
            .collect::<Result<Vec<L1BlockId>, anyhow::Error>>()?;

        Ok(res)
    }

    fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>> {
        Ok(self.db.get::<L1BlockSchema>(&idx)?)
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use strata_primitives::l1::{L1TxProof, ProtocolOperation};
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn setup_db() -> L1Db {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        L1Db::new(db, db_ops)
    }

    fn insert_block_data(
        idx: u64,
        db: &L1Db,
        num_txs: usize,
    ) -> (L1BlockManifest, Vec<L1Tx>, CompactMmr) {
        let mut arb = ArbitraryGenerator::new_with_size(1 << 12);

        // TODO maybe tweak this to make it a bit more realistic?
        let mf: L1BlockManifest = arb.generate();
        let txs: Vec<L1Tx> = (0..num_txs)
            .map(|i| {
                let proof = L1TxProof::new(i as u32, arb.generate());
                let parsed_tx: ProtocolOperation = arb.generate();
                L1Tx::new(proof, arb.generate(), vec![parsed_tx])
            })
            .collect();
        let mmr: CompactMmr = arb.generate();

        // Insert block data
        let res = db.put_block_data(idx, mf.clone(), txs.clone());
        assert!(res.is_ok(), "put should work but got: {}", res.unwrap_err());

        // Insert mmr data
        db.put_mmr_checkpoint(idx, mmr.clone()).unwrap();

        (mf, txs, mmr)
    }

    // TEST STORE METHODS

    #[test]
    fn test_insert_into_empty_db() {
        let db = setup_db();
        let idx = 1;
        insert_block_data(idx, &db, 10);
        drop(db);

        // insert another block with arbitrary id
        let db = setup_db();
        let idx = 200_011;
        insert_block_data(idx, &db, 10);
    }

    #[test]
    fn test_insert_into_non_empty_db() {
        let db = setup_db();
        let idx = 1_000;
        insert_block_data(idx, &db, 10); // first insertion

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
        let num_txs = 10;
        let _ = insert_block_data(1, &db, num_txs);
        let _ = insert_block_data(2, &db, num_txs);
        let _ = insert_block_data(3, &db, num_txs);
        let _ = insert_block_data(4, &db, num_txs);

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
        let num_txs = 10;
        let _ = insert_block_data(1, &db, num_txs);
        let _ = insert_block_data(2, &db, num_txs);
        let _ = insert_block_data(3, &db, num_txs);
        let _ = insert_block_data(4, &db, num_txs);

        let res = db.revert_to_height(0);
        assert!(res.is_ok(), "Should succeed to revert to height 0");
    }

    #[test]
    fn test_revert_to_non_zero_height() {
        let db = setup_db();
        // First insert a couple of manifests
        let num_txs = 10;
        let _ = insert_block_data(1, &db, num_txs);
        let _ = insert_block_data(2, &db, num_txs);
        let _ = insert_block_data(3, &db, num_txs);
        let _ = insert_block_data(4, &db, num_txs);

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
        let _ = insert_block_data(1, &db, 10);
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
        let _ = insert_block_data(1, &db, 10);
        let mmr: CompactMmr = ArbitraryGenerator::new().generate();
        let res = db.put_mmr_checkpoint(1, mmr);
        assert!(res.is_ok());
    }

    // TEST PROVIDER METHODS

    #[test]
    fn test_get_block_data() {
        let db = setup_db();
        let idx = 1;

        // insert
        let (mf, txs, _) = insert_block_data(idx, &db, 10);

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
        let (_, txns, _) = insert_block_data(idx, &db, 10);
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
            None,
            "chain tip of empty db should be unset"
        );

        // Insert some block data
        let num_txs = 10;
        insert_block_data(1, &db, num_txs);
        assert_eq!(db.get_chain_tip().unwrap(), Some(1));
        insert_block_data(2, &db, num_txs);
        assert_eq!(db.get_chain_tip().unwrap(), Some(2));
    }

    #[test]
    fn test_get_block_txs() {
        let db = setup_db();

        let num_txs = 10;
        insert_block_data(1, &db, num_txs);
        insert_block_data(2, &db, num_txs);
        insert_block_data(3, &db, num_txs);

        let block_txs = db.get_block_txs(2).unwrap().unwrap();
        let expected: Vec<_> = (0..10).map(|i| (2, i).into()).collect(); // 10 because insert_block_data inserts 10 txs
        assert_eq!(block_txs, expected);
    }

    #[test]
    fn test_get_blockid_invalid_range() {
        let db = setup_db();

        let num_txs = 10;
        let _ = insert_block_data(1, &db, num_txs);
        let _ = insert_block_data(2, &db, num_txs);
        let _ = insert_block_data(3, &db, num_txs);

        let range = db.get_blockid_range(3, 1).unwrap();
        assert_eq!(range.len(), 0);
    }

    #[test]
    fn test_get_blockid_range() {
        let db = setup_db();

        let num_txs = 10;
        let (mf1, _, _) = insert_block_data(1, &db, num_txs);
        let (mf2, _, _) = insert_block_data(2, &db, num_txs);
        let (mf3, _, _) = insert_block_data(3, &db, num_txs);

        let range = db.get_blockid_range(1, 4).unwrap();
        assert_eq!(range.len(), 3);
        for (exp, obt) in vec![mf1, mf2, mf3].iter().zip(range) {
            assert_eq!(*exp.blkid(), obt);
        }
    }

    #[test]
    fn test_get_last_mmr_to() {
        let db = setup_db();

        let inexistent_idx = 3;
        let mmr = db.get_last_mmr_to(inexistent_idx).unwrap();
        assert!(mmr.is_none());
        let (_, _, mmr) = insert_block_data(1, &db, 10);
        let mmr_res = db.get_last_mmr_to(inexistent_idx).unwrap();
        assert!(mmr_res.is_none());

        // existent mmr
        let observed_mmr = db.get_last_mmr_to(1).unwrap();
        assert_eq!(Some(mmr), observed_mmr);
    }

    #[test]
    fn test_get_txs_fancy() {
        let db = setup_db();

        let num_txs = 3;
        let total_num_blocks = 4;

        let mut l1_txs = Vec::with_capacity(total_num_blocks);
        for i in 0..total_num_blocks {
            let (_, block_txs, _) = insert_block_data(i as u64, &db, num_txs);
            l1_txs.push(block_txs);
        }

        let latest_idx = db
            .get_latest_block_number()
            .expect("should not error")
            .expect("should have latest");

        assert_eq!(
            latest_idx,
            (total_num_blocks - 1) as u64,
            "the latest index must match the total number of blocks inserted"
        );

        for (block_num, block_txs) in l1_txs.iter().enumerate() {
            for (i, exp_tx) in block_txs.iter().enumerate() {
                let real_tx = db
                    .get_tx(L1TxRef::from((block_num as u64, i as u32)))
                    .expect("test: database failed")
                    .expect("test: missing expected tx");

                assert_eq!(
                    &real_tx, exp_tx,
                    "tx mismatch in block {block_num} at idx {i}"
                );
            }
        }

        // get past the final index.
        let latest_idx = db
            .get_latest_block_number()
            .expect("should not error")
            .expect("should have latest");
        let expected_latest = (total_num_blocks - 1) as u64;

        assert_eq!(
            latest_idx, expected_latest,
            "test: wrong latest block number",
        );
    }
}
