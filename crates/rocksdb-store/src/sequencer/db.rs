use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};
use strata_db::{
    errors::DbError,
    traits::L1PayloadDatabase,
    types::{IntentEntry, PayloadEntry},
    DbResult,
};
use strata_primitives::buf::Buf32;

use super::schemas::{IntentSchema, PayloadSchema};
use crate::DbOpsConfig;

pub struct RBPayloadDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl RBPayloadDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl L1PayloadDatabase for RBPayloadDb {
    fn put_payload_entry(&self, idx: u64, entry: PayloadEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |tx| -> Result<(), DbError> {
                    tx.put::<PayloadSchema>(&idx, &entry)?;
                    Ok(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_payload_entry_by_idx(&self, idx: u64) -> DbResult<Option<PayloadEntry>> {
        Ok(self.db.get::<PayloadSchema>(&idx)?)
    }

    fn get_next_payload_idx(&self) -> DbResult<u64> {
        Ok(rockbound::utils::get_last::<PayloadSchema>(&*self.db)?
            .map(|(x, _)| x + 1)
            .unwrap_or(0))
    }

    fn put_intent_entry(&self, intent_id: Buf32, intent_entry: IntentEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |tx| -> Result<(), DbError> {
                    tx.put::<IntentSchema>(&intent_id, &intent_entry)?;

                    Ok(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_intent_by_id(&self, id: Buf32) -> DbResult<Option<IntentEntry>> {
        Ok(self.db.get::<IntentSchema>(&id)?)
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
        let seq_db = RBPayloadDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();
        let idx = seq_db.get_next_payload_idx().unwrap().unwrap();

        assert_eq!(seq_db.get_payload_id(idx).unwrap(), Some(blob_hash));

        let stored_blob = seq_db.get_payload_entry_by_idx(blob_hash).unwrap();
        assert_eq!(stored_blob, Some(blob));
    }

    #[test]
    fn test_put_blob_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBPayloadDb::new(db, db_ops);
        let blob: PayloadEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        seq_db.put_payload_entry(blob_hash, blob.clone()).unwrap();

        let result = seq_db.put_payload_entry(blob_hash, blob);

        // Should be ok to put to existing key
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBPayloadDb::new(db, db_ops);

        let entry: PayloadEntry = ArbitraryGenerator::new().generate();

        // Insert
        seq_db.put_payload_entry(0, entry.clone()).unwrap();

        let updated_entry: PayloadEntry = ArbitraryGenerator::new().generate();

        // Update existing idx
        seq_db.put_payload_entry(0, updated_entry.clone()).unwrap();
        let retrieved_entry = seq_db.get_payload_entry_by_idx(0).unwrap().unwrap();
        assert_eq!(updated_entry, retrieved_entry);
    }

    #[test]
    fn test_get_last_entry_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBPayloadDb::new(db, db_ops);

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();

        let next_blob_idx = seq_db.get_next_payload_idx().unwrap();
        assert_eq!(
            next_blob_idx, 0,
            "There is no last blobidx in the beginning"
        );

        seq_db
            .put_payload_entry(next_blob_idx, blob.clone())
            .unwrap();
        let next_blob_idx = seq_db.get_next_payload_idx().unwrap();
        // Now the next idx is 1

        let blob: PayloadEntry = ArbitraryGenerator::new().generate();

        seq_db.put_payload_entry(1, blob.clone()).unwrap();
        let next_blob_idx = seq_db.get_next_payload_idx().unwrap();
        // Now the last idx is 2

        assert_eq!(next_blob_idx, 2);
    }

    // Intent related tests

    #[test]
    fn test_put_intent_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBPayloadDb::new(db, db_ops);

        let intent: IntentEntry = ArbitraryGenerator::new().generate();
        let intent_id: Buf32 = [0; 32].into();

        seq_db.put_intent_entry(intent_id, intent.clone()).unwrap();

        let stored_intent = seq_db.get_intent_by_id(intent_id).unwrap();
        assert_eq!(stored_intent, Some(intent));
    }

    #[test]
    fn test_put_intent_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBPayloadDb::new(db, db_ops);
        let intent: IntentEntry = ArbitraryGenerator::new().generate();
        let intent_id: Buf32 = [0; 32].into();

        let result = seq_db.put_intent_entry(intent_id, intent.clone());
        assert!(result.is_ok());

        let retrieved = seq_db.get_intent_by_id(intent_id).unwrap().unwrap();
        assert_eq!(retrieved, intent);
    }
}
