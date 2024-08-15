use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaBatch, SchemaDBOperationsExt};

use alpen_express_db::{
    errors::DbError,
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobEntry,
    DbResult,
};
use alpen_express_primitives::buf::Buf32;

use super::schemas::{SeqBlobIdSchema, SeqBlobSchema, SeqL1TxnSchema};
use crate::DbOpsConfig;

pub struct SeqDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl SeqDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl SeqDataStore for SeqDb {
    fn put_blob(&self, blob_hash: Buf32, blob: BlobEntry) -> DbResult<u64> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |txn| {
                    if txn.get::<SeqBlobSchema>(&blob_hash)?.is_some() {
                        return Err(DbError::Other(format!(
                            "Entry already exists for blobid {blob_hash:?}"
                        )));
                    }

                    let idx = rockbound::utils::get_last::<SeqBlobIdSchema>(txn)?
                        .map(|(x, _)| x + 1)
                        .unwrap_or(0);

                    txn.put::<SeqBlobIdSchema>(&idx, &blob_hash)?;
                    txn.put::<SeqBlobSchema>(&blob_hash, &blob)?;

                    Ok(idx)
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn put_commit_reveal_txs(
        &self,
        commit_txid: Buf32,
        commit_tx: Vec<u8>,
        reveal_txid: Buf32,
        reveal_tx: Vec<u8>,
    ) -> DbResult<()> {
        let mut batch = SchemaBatch::new();

        // Atomically add the entries
        batch.put::<SeqL1TxnSchema>(&commit_txid, &commit_tx)?;
        batch.put::<SeqL1TxnSchema>(&reveal_txid, &reveal_tx)?;

        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn update_blob_by_idx(&self, blobidx: u64, blobentry: BlobEntry) -> DbResult<()> {
        match self.db.get::<SeqBlobIdSchema>(&blobidx)? {
            Some(id) => Ok(self.db.put::<SeqBlobSchema>(&id, &blobentry)?),
            None => Err(DbError::Other(format!(
                "BlobEntry does not exist for idx {blobidx:?}"
            ))),
        }
    }
}

impl SeqDataProvider for SeqDb {
    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<BlobEntry>> {
        Ok(self.db.get::<SeqBlobSchema>(&id)?)
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<SeqBlobIdSchema>(&*self.db)?.map(|(x, _)| x))
    }

    fn get_l1_tx(&self, txid: Buf32) -> DbResult<Option<Vec<u8>>> {
        Ok(self.db.get::<SeqL1TxnSchema>(&txid)?)
    }

    fn get_blob_by_idx(&self, blobidx: u64) -> DbResult<Option<BlobEntry>> {
        match self.db.get::<SeqBlobIdSchema>(&blobidx)? {
            Some(id) => Ok(self.db.get::<SeqBlobSchema>(&id)?),
            None => Ok(None),
        }
    }
}

pub struct SequencerDB<D> {
    db: Arc<D>,
}

impl<D> SequencerDB<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}

impl<D: SeqDataStore + SeqDataProvider> SequencerDatabase for SequencerDB<D> {
    type SeqStore = D;
    type SeqProv = D;

    fn sequencer_store(&self) -> &Arc<Self::SeqStore> {
        &self.db
    }

    fn sequencer_provider(&self) -> &Arc<Self::SeqProv> {
        &self.db
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use bitcoin::consensus::serialize;
    use bitcoin::hashes::Hash;

    use alpen_express_db::errors::DbError;
    use alpen_express_db::traits::{SeqDataProvider, SeqDataStore};
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::bitcoin::get_test_bitcoin_txns;
    use alpen_test_utils::ArbitraryGenerator;

    use crate::test_utils::get_rocksdb_tmp_instance;

    use super::*;

    use test;

    fn get_commit_reveal_txns() -> ((Buf32, Vec<u8>), (Buf32, Vec<u8>)) {
        let txns = get_test_bitcoin_txns();
        let ctxid = txns[0].compute_txid().as_raw_hash().to_byte_array().into();
        let rtxid = txns[1].compute_txid().as_raw_hash().to_byte_array().into();
        ((ctxid, serialize(&txns[0])), (rtxid, serialize(&txns[1])))
    }

    #[test]
    fn test_put_blob_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let idx = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        assert_eq!(idx, 0);

        let stored_blob = seq_db.get_blob_by_idx(idx).unwrap();
        assert_eq!(stored_blob, Some(blob));
    }

    #[test]
    fn test_put_blob_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);
        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let _ = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let result = seq_db.put_blob(blob_hash, blob);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for blobid"));
        }
    }

    #[test]
    fn test_put_commit_reveal_txns() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let ((cid, craw), (rid, rraw)) = get_commit_reveal_txns();

        seq_db
            .put_commit_reveal_txs(cid, craw.clone(), rid, rraw.clone())
            .unwrap();

        let stored_commit_txn = seq_db.get_l1_tx(cid).unwrap();
        assert_eq!(stored_commit_txn, Some(craw));

        let stored_reveal_txn = seq_db.get_l1_tx(rid).unwrap();
        assert_eq!(stored_reveal_txn, Some(rraw));
    }

    #[test]
    fn test_update_blob_by_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        // Insert
        let idx = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        // Try update inexistent idx
        let res = seq_db.update_blob_by_idx(idx + 1, blob);
        assert!(res.is_err());

        let updated_blob: BlobEntry = ArbitraryGenerator::new().generate();

        // Update existing idx
        seq_db
            .update_blob_by_idx(idx, updated_blob.clone())
            .unwrap();
        let retrieved_blob = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
        assert_eq!(updated_blob, retrieved_blob);
    }

    #[test]
    fn test_get_blob_by_id() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let _ = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let retrieved = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
        assert_eq!(retrieved, blob);
    }

    #[test]
    fn test_get_blob_by_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let idx = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let retrieved = seq_db.get_blob_by_idx(idx).unwrap().unwrap();
        assert_eq!(retrieved, blob);
    }

    #[test]
    fn test_get_last_blob_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let last_blob_idx = seq_db.get_last_blob_idx().unwrap();
        assert_eq!(
            last_blob_idx, None,
            "There is no last blobidx in the beginning"
        );

        let _ = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [1; 32].into();

        let idx = seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let last_blob_idx = seq_db.get_last_blob_idx().unwrap();

        assert_eq!(last_blob_idx, Some(idx));
    }

    #[test]
    fn test_get_l1_tx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        // Test non existing l1 tx
        let res = seq_db.get_l1_tx(Buf32::zero()).unwrap();
        assert_eq!(res, None);

        let ((cid, craw), (rid, rraw)) = get_commit_reveal_txns();

        seq_db
            .put_commit_reveal_txs(cid, craw.clone(), rid, rraw.clone())
            .unwrap();

        let stored_commit_txn = seq_db.get_l1_tx(cid).unwrap();
        assert_eq!(stored_commit_txn, Some(craw));

        let stored_reveal_txn = seq_db.get_l1_tx(rid).unwrap();
        assert_eq!(stored_reveal_txn, Some(rraw));
    }
}
