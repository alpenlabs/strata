use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};
use strata_db::{traits::CheckpointDatabase, types::CheckpointEntry, DbResult};

use super::schemas::CheckpointSchema;
use crate::DbOpsConfig;

pub struct RBCheckpointDB {
    db: Arc<OptimisticTransactionDB>,
    #[allow(dead_code)]
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

impl CheckpointDatabase for RBCheckpointDB {
    fn put_batch_checkpoint(
        &self,
        batchidx: u64,
        batch_checkpoint: CheckpointEntry,
    ) -> DbResult<()> {
        Ok(self
            .db
            .put::<CheckpointSchema>(&batchidx, &batch_checkpoint)?)
    }

    fn get_batch_checkpoint(&self, batchidx: u64) -> DbResult<Option<CheckpointEntry>> {
        Ok(self.db.get::<CheckpointSchema>(&batchidx)?)
    }

    fn get_last_batch_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<CheckpointSchema>(&*self.db)?.map(|(x, _)| x))
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use strata_test_utils::ArbitraryGenerator;
    use test;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_batch_checkpoint_new_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batchidx = 1;
        let checkpoint: CheckpointEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_checkpoint(batchidx, checkpoint.clone())
            .unwrap();

        let retrieved_batch = seq_db.get_batch_checkpoint(batchidx).unwrap().unwrap();
        assert_eq!(checkpoint, retrieved_batch);
    }

    #[test]
    fn test_batch_checkpoint_existing_entry() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let batchidx = 1;
        let checkpoint: CheckpointEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_checkpoint(batchidx, checkpoint.clone())
            .unwrap();

        seq_db
            .put_batch_checkpoint(batchidx, checkpoint.clone())
            .unwrap();
    }

    #[test]
    fn test_batch_checkpoint_non_monotonic_entries() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let checkpoint: CheckpointEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_checkpoint(100, checkpoint.clone())
            .unwrap();
        seq_db.put_batch_checkpoint(1, checkpoint.clone()).unwrap();
        seq_db.put_batch_checkpoint(3, checkpoint.clone()).unwrap();
    }

    #[test]
    fn test_get_last_batch_checkpoint_idx() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let checkpoint: CheckpointEntry = ArbitraryGenerator::new().generate();
        seq_db
            .put_batch_checkpoint(100, checkpoint.clone())
            .unwrap();
        seq_db.put_batch_checkpoint(1, checkpoint.clone()).unwrap();
        seq_db.put_batch_checkpoint(3, checkpoint.clone()).unwrap();

        let last_idx = seq_db.get_last_batch_idx().unwrap().unwrap();
        assert_eq!(last_idx, 100);

        seq_db.put_batch_checkpoint(50, checkpoint.clone()).unwrap();
        let last_idx = seq_db.get_last_batch_idx().unwrap().unwrap();
        assert_eq!(last_idx, 100);
    }

    /// Tests a peculiar issue with `default_codec` in rockbound schema. If it is used instead of
    /// `seek_key_codec`, the last_idx won't grow beyond 255.
    #[test]
    fn test_256_checkpoints() {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seq_db = RBCheckpointDB::new(db, db_ops);

        let checkpoint: CheckpointEntry = ArbitraryGenerator::new().generate();

        for expected_idx in 0..=256 {
            let last_idx = seq_db.get_last_batch_idx().unwrap().unwrap_or(0);
            assert_eq!(last_idx, expected_idx);

            // Insert one to db
            seq_db
                .put_batch_checkpoint(last_idx + 1, checkpoint.clone())
                .unwrap();
        }
    }
}
