use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{ProverDataProvider, ProverDataStore, ProverDatabase},
    DbResult,
};
use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt};

use super::schemas::ProverTaskSchema;
use crate::DbOpsConfig;

pub struct ProofDb {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl ProofDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl ProverDataStore for ProofDb {
    fn put_blob_entry(&self, blob_hash: u64, blob: Vec<u8>) -> DbResult<()> {
        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |tx| -> Result<(), DbError> {
                    // If new, increment idx
                    if tx.get::<ProverTaskSchema>(&blob_hash)?.is_none() {
                        let idx = rockbound::utils::get_last::<ProverTaskSchema>(tx)?
                            .map(|(x, _)| x + 1)
                            .unwrap_or(0);

                        tx.put::<ProverTaskSchema>(&idx, &blob)?;
                    }

                    tx.put::<ProverTaskSchema>(&blob_hash, &blob)?;

                    Ok(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }
}

impl ProverDataProvider for ProofDb {
    fn get_blob_by_id(&self, id: u64) -> DbResult<Option<Vec<u8>>> {
        Ok(self.db.get::<ProverTaskSchema>(&id)?)
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        Ok(rockbound::utils::get_last::<ProverTaskSchema>(&*self.db)?.map(|(x, _)| x))
    }

    fn get_blob_id(&self, blobidx: u64) -> DbResult<Option<Vec<u8>>> {
        Ok(self.db.get::<ProverTaskSchema>(&blobidx)?)
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
//     use alpen_express_primitives::buf::Buf32;
//     use alpen_test_utils::ArbitraryGenerator;
//     use test;

//     use super::*;
//     use crate::test_utils::get_rocksdb_tmp_instance;

//     #[test]
//     fn test_put_blob_new_entry() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: Buf32 = [0; 32].into();

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
//         let blob_hash: Buf32 = [0; 32].into();

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
//         let blob_hash: Buf32 = [0; 32].into();

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
//         let blob_hash: Buf32 = [0; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();

//         let retrieved = seq_db.get_blob_by_id(blob_hash).unwrap().unwrap();
//         assert_eq!(retrieved, blob);
//     }

//     #[test]
//     fn test_get_last_blob_idx() {
//         let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
//         let seq_db = SeqDb::new(db, db_ops);

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: Buf32 = [0; 32].into();

//         let last_blob_idx = seq_db.get_last_blob_idx().unwrap();
//         assert_eq!(
//             last_blob_idx, None,
//             "There is no last blobidx in the beginning"
//         );

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();
//         // Now the last idx is 0

//         let blob: BlobEntry = ArbitraryGenerator::new().generate();
//         let blob_hash: Buf32 = [1; 32].into();

//         seq_db.put_blob_entry(blob_hash, blob.clone()).unwrap();
//         // Now the last idx is 1

//         let last_blob_idx = seq_db.get_last_blob_idx().unwrap();
//         assert_eq!(last_blob_idx, Some(1));
//     }
// }
