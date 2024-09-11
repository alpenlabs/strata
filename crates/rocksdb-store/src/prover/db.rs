use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{ProverDataProvider, ProverDataStore, ProverDatabase},
    DbResult,
};
use rockbound::{
    utils::get_last, OptimisticTransactionDB, SchemaDBOperationsExt, TransactionRetry,
};

use super::schemas::{ProverTaskIdSchema, ProverTaskSchema};
use crate::DbOpsConfig;

pub struct ProofDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl ProofDb {
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl ProverDataStore for ProofDb {
    fn insert_new_task_entry(&self, taskid: [u8; 16], taskentry: Vec<u8>) -> DbResult<u64> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProverTaskSchema>(&taskid)?.is_some() {
                    return Err(DbError::Other(format!(
                        "Entry already exists for id {taskid:?}"
                    )));
                }

                let idx = rockbound::utils::get_last::<ProverTaskIdSchema>(tx)?
                    .map(|(x, _)| x + 1)
                    .unwrap_or(0);

                tx.put::<ProverTaskIdSchema>(&idx, &taskid)?;
                tx.put::<ProverTaskSchema>(&taskid, &taskentry)?;

                Ok(idx)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn update_task_entry_by_id(&self, taskid: [u8; 16], taskentry: Vec<u8>) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProverTaskSchema>(&taskid)?.is_none() {
                    return Err(DbError::Other(format!(
                        "Entry does not exist for id {taskid:?}"
                    )));
                }
                Ok(tx.put::<ProverTaskSchema>(&taskid, &taskentry)?)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn update_task_entry(&self, idx: u64, taskentry: Vec<u8>) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if let Some(id) = tx.get::<ProverTaskIdSchema>(&idx)? {
                    Ok(tx.put::<ProverTaskSchema>(&id, &taskentry)?)
                } else {
                    Err(DbError::Other(format!(
                        "Entry does not exist for idx {idx:?}"
                    )))
                }
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }
}

impl ProverDataProvider for ProofDb {
    fn get_task_entry_by_id(&self, taskid: [u8; 16]) -> DbResult<Option<Vec<u8>>> {
        Ok(self.db.get::<ProverTaskSchema>(&taskid)?)
    }

    fn get_next_task_idx(&self) -> DbResult<u64> {
        Ok(get_last::<ProverTaskIdSchema>(self.db.as_ref())?
            .map(|(k, _)| k + 1)
            .unwrap_or_default())
    }

    fn get_taskid(&self, idx: u64) -> DbResult<Option<[u8; 16]>> {
        Ok(self.db.get::<ProverTaskIdSchema>(&idx)?)
    }

    fn get_task_entry(&self, idx: u64) -> DbResult<Option<Vec<u8>>> {
        if let Some(id) = self.get_taskid(idx)? {
            Ok(self.db.get::<ProverTaskSchema>(&id)?)
        } else {
            Err(DbError::Other(format!(
                "Entry does not exist for idx {idx:?}"
            )))
        }
    }
}

pub struct ProverDB<D> {
    db: Arc<D>,
}

impl<D> ProverDB<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}

impl<D: ProverDataStore + ProverDataProvider> ProverDatabase for ProverDB<D> {
    type ProverStore = D;
    type ProverProv = D;

    fn prover_store(&self) -> &Arc<Self::ProverStore> {
        &self.db
    }

    fn prover_provider(&self) -> &Arc<Self::ProverProv> {
        &self.db
    }
}

// #[cfg(feature = "test_utils")]
// #[cfg(test)]
// mod tests {
//     use alpen_express_db::traits::{ProverDataProvider, ProverDataStore};
//     use alpen_express_primitives::buf::[u8;16];
//     use alpen_test_utils::ArbitraryGenerator;
//     use test;

//     use super::*;
//     use crate::test_utils::get_rocksdb_tmp_instance;

//     #[test]
//     fn test_put_blob_new_entry() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [0; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();
//         let idx = seq_db.get_last_blob_idx().unwrap().unwrap();

//         assert_eq!(seq_db.get_blob_id(idx).unwrap(), Some(blob_hash));

//         let stored_blob = seq_db.get_blob_by_id(blob_hash).unwrap();
//         assert_eq!(stored_blob, Some(blob));
//     }

//     #[test]
//     fn test_put_blob_existing_entry() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);
//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [0; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();

//         let result = seq_db.put_blob_entry(blob_hash, blob);

//         // Should be ok to put to existing key
//         assert!(result.is_ok());
//     }

//     #[test]
//     fn test_update_blob_() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [0; 32].into();

//         // Insert
//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();

//         let updated_blob: BlobEntry = ArbitraryGenerator::new().generate();

//         // Update existing idx
//         seq_db
//             .put_blob_entry(blob_hash, updated_blob.clone())
//             .unwrap();
//         let retrieved_blob = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
//         assert_eq!(updated_blob, retrieved_blob);
//     }

//     #[test]
//     fn test_get_blob_by_id() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [0; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();

//         let retrieved = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
//         assert_eq!(retrieved, blob);
//     }

//     #[test]
//     fn test_get_last_blob_idx() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [0; 32].into();

//         let last_blob_idx = seq_db.get_last_blob_idx().unwrap();
//         assert_eq!(
//             last_blob_idx, None,
//             "There is no last blobidx in the beginning"
//         );

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();
//         // Now the last idx is 0

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: [u8;16] = [1; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();
//         // Now the last idx is 1

//         let last_blob_idx = seq_db.get_last_blob_idx().unwrap();
//         assert_eq!(last_blob_idx, Some(1));
//     }
// }
