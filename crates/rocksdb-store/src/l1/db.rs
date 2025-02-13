use std::sync::Arc;

use rockbound::{
    rocksdb::ReadOptions,
    schema::KeyEncoder,
    utils::{get_first, get_last},
    OptimisticTransactionDB, SchemaBatch, SchemaDBOperationsExt,
};
use strata_db::{errors::DbError, traits::*, DbResult};
use strata_mmr::CompactMmr;
use strata_primitives::l1::{L1BlockId, L1BlockManifest, L1Tx, L1TxRef};
use tracing::*;

use super::schemas::{
    L1BlockSchema, L1BlocksByHeightSchema, L1CanonicalBlockSchema, MmrSchema, TxnSchema,
};
use crate::DbOpsConfig;

pub struct L1Db {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl L1Db {
    // NOTE: db is expected to open all the column families defined in STORE_COLUMN_FAMILIES.
    // FIXME: Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }

    pub fn get_latest_block(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        Ok(get_last::<L1CanonicalBlockSchema>(self.db.as_ref())?)
    }
}

impl L1Database for L1Db {
    fn put_block_data(&self, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()> {
        let blockid = mf.blkid();
        let height = mf.height();

        self.db
            .with_optimistic_txn(self.ops.txn_retry_count(), |txn| {
                let mut blocks_at_height = txn
                    .get_for_update::<L1BlocksByHeightSchema>(&height)?
                    .unwrap_or_default();
                blocks_at_height.push(*blockid);

                let mut batch = SchemaBatch::new();
                batch.put::<L1BlockSchema>(blockid, &mf)?;
                batch.put::<TxnSchema>(blockid, &txs)?;
                batch.put::<L1BlocksByHeightSchema>(&height, &blocks_at_height)?;
                txn.write_schemas(batch)
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }

    fn put_mmr_checkpoint(&self, blockid: L1BlockId, mmr: CompactMmr) -> DbResult<()> {
        self.db.put::<MmrSchema>(&blockid, &mmr)?;
        Ok(())
    }

    fn add_at_height(&self, height: u64, blockid: L1BlockId) -> DbResult<()> {
        match self.get_latest_block()? {
            // if block exists, new block must extend chain sequentially
            Some((existing_height, _)) if existing_height + 1 != height => {
                return Err(DbError::OooInsert("l1_store", height));
            }
            // if no blocks in db, allow block insert at any height
            _ => {}
        }

        self.db.put::<L1CanonicalBlockSchema>(&height, &blockid)?;
        Ok(())
    }

    fn revert_to_height(&self, height: u64) -> DbResult<()> {
        // Get latest height, iterate backwards upto the idx, get blockhash and delete txns and
        // blockmanifest data at each iteration
        let last_height = self
            .get_latest_block()?
            .map(|(height, _)| height)
            .unwrap_or(0);
        if height > last_height {
            return Err(DbError::Other(
                "Invalid block number to revert to".to_string(),
            ));
        }

        let mut batch = SchemaBatch::new();
        for height in ((height + 1)..=last_height).rev() {
            batch.delete::<L1CanonicalBlockSchema>(&height)?;
        }

        // Execute the batch
        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn prune_to_height(&self, end_height: u64) -> DbResult<()> {
        let earliest =
            get_first::<L1BlocksByHeightSchema>(self.db.as_ref())?.map(|(height, _)| height);
        let Some(start_height) = earliest else {
            // empty db
            return Ok(());
        };

        for height in start_height..=end_height {
            self.db
                .with_optimistic_txn(self.ops.txn_retry_count(), |txn| {
                    let blocks = txn.get_for_update::<L1BlocksByHeightSchema>(&height)?;
                    let mut batch = SchemaBatch::new();
                    batch.delete::<L1BlocksByHeightSchema>(&height)?;
                    batch.delete::<L1CanonicalBlockSchema>(&height)?;
                    for blockid in blocks.unwrap_or_default() {
                        batch.delete::<L1BlockSchema>(&blockid)?;
                        batch.delete::<TxnSchema>(&blockid)?;
                        batch.delete::<MmrSchema>(&blockid)?;
                    }

                    txn.write_schemas(batch)
                })
                .map_err(|e: rockbound::TransactionError<_>| {
                    DbError::TransactionError(e.to_string())
                })?;
        }
        Ok(())
    }

    fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
        let (blockid, txindex) = tx_ref.into();
        let tx = self
            .db
            .get::<L1BlockSchema>(&blockid)
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

    fn get_chain_tip(&self) -> DbResult<Option<(u64, L1BlockId)>> {
        self.get_latest_block()
    }

    fn get_block_txs(&self, blockid: L1BlockId) -> DbResult<Option<Vec<L1TxRef>>> {
        let Some(txs) = self.db.get::<TxnSchema>(&blockid)? else {
            warn!(%blockid, "missing L1 block body");
            return Err(DbError::MissingL1BlockBody(blockid));
        };

        let txs_refs = txs
            .into_iter()
            .map(|tx| L1TxRef::from((blockid, tx.proof().position())))
            .collect::<Vec<L1TxRef>>();

        Ok(Some(txs_refs))
    }

    fn get_mmr(&self, blockid: L1BlockId) -> DbResult<Option<CompactMmr>> {
        Ok(self.db.get::<MmrSchema>(&blockid)?)
    }

    // TODO: This should not exist in database level and should be handled by downstream manager
    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<L1BlockId>> {
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(
            KeyEncoder::<L1CanonicalBlockSchema>::encode_key(&start_idx)
                .map_err(|err| DbError::CodecError(err.to_string()))?,
        );
        options.set_iterate_upper_bound(
            KeyEncoder::<L1CanonicalBlockSchema>::encode_key(&end_idx)
                .map_err(|err| DbError::CodecError(err.to_string()))?,
        );

        let res = self
            .db
            .iter_with_opts::<L1CanonicalBlockSchema>(options)?
            .map(|item_result| item_result.map(|item| item.into_tuple().1))
            .collect::<Result<Vec<L1BlockId>, anyhow::Error>>()?;

        Ok(res)
    }

    fn get_blockid(&self, height: u64) -> DbResult<Option<L1BlockId>> {
        Ok(self.db.get::<L1CanonicalBlockSchema>(&height)?)
    }

    fn get_block_manifest(&self, blockid: L1BlockId) -> DbResult<Option<L1BlockManifest>> {
        Ok(self.db.get::<L1BlockSchema>(&blockid)?)
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
        height: u64,
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
        let res = db.put_block_data(mf.clone(), txs.clone());
        assert!(res.is_ok(), "put should work but got: {}", res.unwrap_err());
        let res = db.add_at_height(height, *mf.blkid());
        assert!(res.is_ok(), "put should work but got: {}", res.unwrap_err());

        // Insert mmr data
        db.put_mmr_checkpoint(*mf.blkid(), mmr.clone()).unwrap();

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
            let mf: L1BlockManifest = ArbitraryGenerator::new().generate();
            let blockid = *mf.blkid();
            db.put_block_data(mf, txs).unwrap();
            let res = db.add_at_height(invalid_idx, blockid);
            assert!(res.is_err(), "Should fail to insert to db");
        }

        let valid_idx = idx + 1;
        let txs: Vec<L1Tx> = (0..10)
            .map(|_| ArbitraryGenerator::new().generate())
            .collect();
        let mf: L1BlockManifest = ArbitraryGenerator::new().generate();
        let blockid = *mf.blkid();
        db.put_block_data(mf, txs).unwrap();
        let res = db.add_at_height(valid_idx, blockid);
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
        let revert_blockid = db.get_blockid(3).unwrap().unwrap();

        // Check that some txns and mmrs exists upto this height
        for h in 1..=3 {
            let blockid = db.get_blockid(h).unwrap().unwrap();
            let txn_data = db.get_tx((blockid, 0).into()).unwrap();
            assert!(txn_data.is_some());
            let mmr_data = db.get_mmr(blockid).unwrap();
            assert!(mmr_data.is_some());
        }

        // Check that latest block is at revert height
        let latest = db.get_latest_block().unwrap();
        assert!(matches!(latest, Some((3, blockid)) if blockid == revert_blockid));
        // check that block beyond revert height is not accessible
        let blockid = db.get_blockid(4).unwrap();
        assert!(blockid.is_none());
    }

    #[test]
    fn test_put_mmr_checkpoint_valid() {
        let db = setup_db();
        let (mf, _, _) = insert_block_data(1, &db, 10);
        let mmr: CompactMmr = ArbitraryGenerator::new().generate();
        let res = db.put_mmr_checkpoint(*mf.blkid(), mmr);
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
        let observed_blockid = db.get_blockid(non_idx).expect("Could not fetch from db");
        assert_eq!(observed_blockid, None);

        // fetch and check, existent block
        let blockid = db
            .get_blockid(idx)
            .expect("Could not fetch from db")
            .expect("Expected block missing");
        let observed_mf = db
            .get_block_manifest(blockid)
            .expect("Could not fetch from db");
        assert_eq!(observed_mf, Some(mf));

        // Fetch txs
        for (i, tx) in txs.iter().enumerate() {
            let tx_from_db = db
                .get_tx((blockid, i as u32).into())
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
        let (mf, txns, _) = insert_block_data(idx, &db, 10);
        let blockid = mf.blkid();
        let txidx: u32 = 3; // some tx index
        assert!(txns.len() > txidx as usize);
        let tx_ref: L1TxRef = (*blockid, txidx).into();
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
        assert!(matches!(db.get_chain_tip().unwrap(), Some((1, _))));
        insert_block_data(2, &db, num_txs);
        assert!(matches!(db.get_chain_tip().unwrap(), Some((2, _))));
    }

    #[test]
    fn test_get_block_txs() {
        let db = setup_db();

        let num_txs = 10;
        insert_block_data(1, &db, num_txs);
        insert_block_data(2, &db, num_txs);
        insert_block_data(3, &db, num_txs);

        let blockid = db.get_blockid(2).unwrap().unwrap();
        let block_txs = db.get_block_txs(blockid).unwrap().unwrap();
        let expected: Vec<_> = (0..10).map(|i| (blockid, i).into()).collect(); // 10 because insert_block_data inserts 10 txs
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
    fn test_get_mmr() {
        let db = setup_db();

        let (mf, _, mmr) = insert_block_data(1, &db, 10);
        let observed_mmr = db.get_mmr(*mf.blkid()).unwrap();
        assert_eq!(Some(mmr), observed_mmr);
    }

    #[test]
    fn test_get_txs_fancy() {
        let db = setup_db();

        let num_txs = 3;
        let total_num_blocks = 4;

        let mut l1_txs = Vec::with_capacity(total_num_blocks);
        for i in 0..total_num_blocks {
            let (mf, block_txs, _) = insert_block_data(i as u64, &db, num_txs);
            l1_txs.push((*mf.blkid(), block_txs));
        }

        let (latest_idx, _) = db
            .get_latest_block()
            .expect("should not error")
            .expect("should have latest");

        assert_eq!(
            latest_idx,
            (total_num_blocks - 1) as u64,
            "the latest index must match the total number of blocks inserted"
        );

        for (blockid, block_txs) in l1_txs.iter() {
            for (i, exp_tx) in block_txs.iter().enumerate() {
                let real_tx = db
                    .get_tx(L1TxRef::from((*blockid, i as u32)))
                    .expect("test: database failed")
                    .expect("test: missing expected tx");

                assert_eq!(
                    &real_tx, exp_tx,
                    "tx mismatch in block {blockid} at idx {i}"
                );
            }
        }

        // get past the final index.
        let (latest_idx, _) = db
            .get_latest_block()
            .expect("should not error")
            .expect("should have latest");
        let expected_latest = (total_num_blocks - 1) as u64;

        assert_eq!(
            latest_idx, expected_latest,
            "test: wrong latest block number",
        );
    }
}
