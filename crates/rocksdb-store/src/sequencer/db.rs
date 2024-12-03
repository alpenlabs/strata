use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};
use strata_db::{errors::DbError, traits::WriterDatabase, types::DataBundleIntentEntry, DbResult};
use strata_primitives::buf::Buf32;

use super::schemas::{SeqBlobIdSchema, SeqBlobSchema};
use crate::{sequence::get_next_id, DbOpsConfig};

pub struct WriterDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl WriterDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl WriterDatabase for WriterDb {
    fn put_entry(&self, entry_hash: Buf32, entry: DataBundleIntentEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |tx| -> Result<(), DbError> {
                    // If new, increment idx
                    if tx.get::<SeqBlobSchema>(&entry_hash)?.is_none() {
                        let idx = get_next_id::<SeqBlobIdSchema, OptimisticTransactionDB>(tx)?;

                        tx.put::<SeqBlobIdSchema>(&idx, &entry_hash)?;
                    }

                    tx.put::<SeqBlobSchema>(&entry_hash, &entry)?;

                    Ok(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_entry_by_id(&self, id: Buf32) -> DbResult<Option<DataBundleIntentEntry>> {
        Ok(self.db.get::<SeqBlobSchema>(&id)?)
    }

    fn get_last_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<SeqBlobIdSchema>(&*self.db)?.map(|(x, _)| x))
    }

    fn get_id(&self, entryidx: u64) -> DbResult<Option<Buf32>> {
        Ok(self.db.get::<SeqBlobIdSchema>(&entryidx)?)
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use strata_db::traits::WriterDatabase;
    use strata_primitives::buf::Buf32;
    use strata_test_utils::ArbitraryGenerator;
    use test;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_put_blob_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = WriterDb::new(db, db_ops);

        let envelope_entry: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [0; 32].into();

        seq_db
            .put_entry(envelope_hash, envelope_entry.clone())
            .unwrap();
        let idx = seq_db.get_last_idx().unwrap().unwrap();

        assert_eq!(seq_db.get_id(idx).unwrap(), Some(envelope_hash));

        let stored_entry = seq_db.get_entry_by_id(envelope_hash).unwrap();
        assert_eq!(stored_entry, Some(envelope_entry));
    }

    #[test]
    fn test_put_envelope_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = WriterDb::new(db, db_ops);
        let envelope_entry: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [0; 32].into();

        seq_db
            .put_entry(envelope_hash, envelope_entry.clone())
            .unwrap();

        let result = seq_db.put_entry(envelope_hash, envelope_entry);

        // Should be ok to put to existing key
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = WriterDb::new(db, db_ops);

        let envelope: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [0; 32].into();

        // Insert
        seq_db.put_entry(envelope_hash, envelope.clone()).unwrap();

        let updated_envelope: DataBundleIntentEntry = ArbitraryGenerator::new().generate();

        // Update existing idx
        seq_db
            .put_entry(envelope_hash, updated_envelope.clone())
            .unwrap();
        let retrieved_envelope = seq_db.get_entry_by_id(envelope_hash).unwrap().unwrap();
        assert_eq!(updated_envelope, retrieved_envelope);
    }

    #[test]
    fn test_get_envelope_by_id() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = WriterDb::new(db, db_ops);

        let envelope: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [0; 32].into();

        seq_db.put_entry(envelope_hash, envelope.clone()).unwrap();

        let retrieved = seq_db.get_entry_by_id(envelope_hash).unwrap().unwrap();
        assert_eq!(retrieved, envelope);
    }

    #[test]
    fn test_get_last_envelope_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = WriterDb::new(db, db_ops);

        let envelope: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [0; 32].into();

        let last_envelope_idx = seq_db.get_last_idx().unwrap();
        assert_eq!(
            last_envelope_idx, None,
            "There is no last envelopeidx in the beginning"
        );

        seq_db.put_entry(envelope_hash, envelope.clone()).unwrap();
        // Now the last idx is 0

        let envelope: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        let envelope_hash: Buf32 = [1; 32].into();

        seq_db.put_entry(envelope_hash, envelope.clone()).unwrap();
        // Now the last idx is 1

        let last_envelope_idx = seq_db.get_last_idx().unwrap();
        assert_eq!(last_envelope_idx, Some(1));
    }
}
