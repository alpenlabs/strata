use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};
use strata_db::{
    errors::DbError,
    traits::{L1PayloadDatabase, SequencerDatabase},
    types::PayloadEntry,
    DbResult,
};
use strata_primitives::buf::Buf32;

use super::schemas::{SeqBlobIdSchema, SeqBlobSchema};
use crate::{sequence::get_next_id, DbOpsConfig};

pub struct RBSeqBlobDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl RBSeqBlobDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl L1PayloadDatabase for RBSeqBlobDb {
    fn put_payload_entry(&self, payload_id: Buf32, entry: PayloadEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |tx| -> Result<(), DbError> {
                    // If new, increment idx
                    if tx.get::<SeqBlobSchema>(&payload_id)?.is_none() {
                        let idx = get_next_id::<SeqBlobIdSchema, OptimisticTransactionDB>(tx)?;

                        tx.put::<SeqBlobIdSchema>(&idx, &payload_id)?;
                    }

                    tx.put::<SeqBlobSchema>(&payload_id, &entry)?;

                    Ok(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_payload_by_id(&self, id: Buf32) -> DbResult<Option<PayloadEntry>> {
        Ok(self.db.get::<SeqBlobSchema>(&id)?)
    }

    fn get_last_payload_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<SeqBlobIdSchema>(&*self.db)?.map(|(x, _)| x))
    }

    fn get_payload_id(&self, blobidx: u64) -> DbResult<Option<Buf32>> {
        Ok(self.db.get::<SeqBlobIdSchema>(&blobidx)?)
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

impl<B: L1PayloadDatabase> SequencerDatabase for SequencerDB<B> {
    type L1PayloadDB = B;

    fn payload_db(&self) -> &Arc<Self::L1PayloadDB> {
        &self.db
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use strata_db::traits::L1PayloadDatabase;
    use strata_primitives::buf::Buf32;
    use strata_test_utils::ArbitraryGenerator;
    use test;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_put_blob_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBSeqBlobDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();
        let idx = seq_db.get_last_payload_idx().unwrap().unwrap();

        assert_eq!(seq_db.get_payload_id(idx).unwrap(), Some(blob_hash));

        let stored_blob = seq_db.get_payload_by_id(blob_hash).unwrap();
        assert_eq!(stored_blob, Some(blob));
    }

    #[test]
    fn test_put_blob_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBSeqBlobDb::new(db, db_ops);
        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();

        let result = seq_db.put_payload_entry(blob_hash, blob);

        // Should be ok to put to existing key
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_blob_() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBSeqBlobDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        // Insert
        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();

        let updated_blob: PayloadEntry = ArbitraryGenerator::new().generate();

        // Update existing idx
        seq_db
            .put_payload_entry(blob_hash, updated_blob.clone())
            .unwrap();
        let retrieved_blob = seq_db.get_payload_by_id(blob_hash).unwrap().unwrap();
        assert_eq!(updated_blob, retrieved_blob);
    }

    #[test]
    fn test_get_blob_by_id() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBSeqBlobDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();

        let retrieved = seq_db.get_payload_by_id(blob_hash).unwrap().unwrap();
        assert_eq!(retrieved, blob);
    }

    #[test]
    fn test_get_last_blob_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBSeqBlobDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let last_blob_idx = seq_db.get_last_payload_idx().unwrap();
        assert_eq!(
            last_blob_idx, None,
            "There is no last blobidx in the beginning"
        );

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();
        // Now the last idx is 0

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [1; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();
        // Now the last idx is 1

        let last_blob_idx = seq_db.get_last_payload_idx().unwrap();
        assert_eq!(last_blob_idx, Some(1));
    }
}
