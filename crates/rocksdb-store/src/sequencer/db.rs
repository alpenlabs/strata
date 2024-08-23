use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobEntry,
    DbResult,
};
use alpen_express_primitives::buf::Buf32;
use rockbound::{OptimisticTransactionDB, SchemaBatch, SchemaDBOperationsExt};

use super::schemas::{SeqBlobIdSchema, SeqBlobSchema};
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
    fn add_new_blob_entry(&self, blob_hash: Buf32, blob: BlobEntry) -> DbResult<u64> {
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

    fn update_blob_entry(&self, blobid: Buf32, blobentry: BlobEntry) -> DbResult<()> {
        Ok(self.db.put::<SeqBlobSchema>(&blobid, &blobentry)?)
    }
}

impl SeqDataProvider for SeqDb {
    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<BlobEntry>> {
        Ok(self.db.get::<SeqBlobSchema>(&id)?)
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<SeqBlobIdSchema>(&*self.db)?.map(|(x, _)| x))
    }

    fn get_blob_id(&self, blobidx: u64) -> DbResult<Option<Buf32>> {
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
    use alpen_express_db::{
        errors::DbError,
        traits::{SeqDataProvider, SeqDataStore},
    };
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::ArbitraryGenerator;
    use test;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_put_blob_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let idx = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        assert_eq!(idx, 0);
        assert_eq!(seq_db.get_blob_id(idx).unwrap(), Some(blob_hash));

        let stored_blob = seq_db.get_blob_by_id(blob_hash).unwrap();
        assert_eq!(stored_blob, Some(blob));
    }

    #[test]
    fn test_put_blob_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);
        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        let _ = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        let result = seq_db.add_new_blob_entry(blob_hash, blob);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for blobid"));
        }
    }

    #[test]
    fn test_update_blob_() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = SeqDb::new(db, db_ops);

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [0; 32].into();

        // Insert
        let _idx = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        let updated_blob: BlobEntry = ArbitraryGenerator::new().generate();

        // Update existing idx
        seq_db
            .update_blob_entry(blob_hash, updated_blob.clone())
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

        let _ = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        let retrieved = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
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

        let _ = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        let blob: BlobEntry = ArbitraryGenerator::new().generate();
        let blob_hash: Buf32 = [1; 32].into();

        let idx = seq_db.add_new_blob_entry(blob_hash, blob.clone()).unwrap();

        let last_blob_idx = seq_db.get_last_blob_idx().unwrap();

        assert_eq!(last_blob_idx, Some(idx));
    }
}
