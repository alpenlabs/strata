use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{CheckpointDatabase, CheckpointProvider, CheckpointStore},
    types::BatchCommitmentEntry,
    DbResult,
};
use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};

use super::schemas::BatchCommitmentSchema;
use crate::DbOpsConfig;

pub struct RBCheckpointDB {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl RBCheckpointDB {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl CheckpointStore for RBCheckpointDB {
    fn put_batch_commitment(
        &self,
        batchidx: u64,
        batch_commitment: BatchCommitmentEntry,
    ) -> DbResult<()> {
        Ok(self
            .db
            .put::<BatchCommitmentSchema>(&batchidx, &batch_commitment)?)
    }
}

impl CheckpointProvider for RBCheckpointDB {
    fn get_batch_commitment(&self, batchidx: u64) -> DbResult<Option<BatchCommitmentEntry>> {
        Ok(self.db.get::<BatchCommitmentSchema>(&batchidx)?)
    }

    fn get_last_batch_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<BatchCommitmentSchema>(&*self.db)?.map(|(x, _)| x))
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use alpen_test_utils::ArbitraryGenerator;
    use test;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_batch_commitment_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batchidx = 1;
        let batch: BatchCommitmentEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_commitment(batchidx, batch.clone())
            .unwrap();

        let retrieved_batch = seq_db.get_batch_commitment(batchidx).unwrap().unwrap();
        assert_eq!(batch, retrieved_batch);
    }

    #[test]
    fn test_batch_commitment_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batchidx = 1;
        let batch: BatchCommitmentEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_commitment(batchidx, batch.clone())
            .unwrap();

        seq_db
            .put_batch_commitment(batchidx, batch.clone())
            .unwrap();
    }

    #[test]
    fn test_batch_commitment_non_monotonic_entries() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batch: BatchCommitmentEntry = ArbitraryGenerator::new().generate();
        seq_db.put_batch_commitment(100, batch.clone()).unwrap();
        seq_db.put_batch_commitment(1, batch.clone()).unwrap();
        seq_db.put_batch_commitment(3, batch.clone()).unwrap();
    }

    #[test]
    fn test_get_last_batch_commitment_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batch: BatchCommitmentEntry = ArbitraryGenerator::new().generate();
        seq_db.put_batch_commitment(100, batch.clone()).unwrap();
        seq_db.put_batch_commitment(1, batch.clone()).unwrap();
        seq_db.put_batch_commitment(3, batch.clone()).unwrap();

        let last_idx = seq_db.get_last_batch_idx().unwrap().unwrap();
        assert_eq!(last_idx, 100);

        seq_db.put_batch_commitment(50, batch.clone()).unwrap();
        let last_idx = seq_db.get_last_batch_idx().unwrap().unwrap();
        assert_eq!(last_idx, 100);
    }
}
