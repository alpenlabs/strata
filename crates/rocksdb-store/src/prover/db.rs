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

#[cfg(test)]
mod tests {
    use alpen_express_db::{
        errors::DbError,
        traits::{ProverDataProvider, ProverDataStore},
    };

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance_for_prover;

    fn setup_db() -> ProofDb {
        let (db, db_ops) = get_rocksdb_tmp_instance_for_prover().unwrap();
        ProofDb::new(db, db_ops)
    }

    fn generate_l1_task_entry() -> ([u8; 16], Vec<u8>) {
        let txid = [1u8; 16];
        let txentry = vec![1u8; 64];
        (txid, txentry)
    }

    #[test]
    fn test_add_tx_new_entry() {
        let db = setup_db();

        let (txid, txentry) = generate_l1_task_entry();

        let idx = db.insert_new_task_entry(txid, txentry.clone()).unwrap();

        assert_eq!(idx, 0);

        let stored_entry = db.get_task_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_add_tx_existing_entry() {
        let proof_db = setup_db();

        let (txid, txentry) = generate_l1_task_entry();

        let _ = proof_db
            .insert_new_task_entry(txid, txentry.clone())
            .unwrap();

        let result = proof_db.insert_new_task_entry(txid, txentry);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for id"));
        }
    }

    #[test]
    fn test_update_tx() {
        let proof_db = setup_db();

        let (txid, txentry) = generate_l1_task_entry();

        // Attempt to update non-existing entry
        let result = proof_db.update_task_entry_by_id(txid, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry
        let _ = proof_db
            .insert_new_task_entry(txid, txentry.clone())
            .unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.push(2u8);

        proof_db
            .update_task_entry_by_id(txid, updated_txentry.clone())
            .unwrap();

        let stored_entry = proof_db.get_task_entry_by_id(txid).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_update_task_entry() {
        let proof_db = setup_db();

        let (txid, txentry) = generate_l1_task_entry();

        // Attempt to update non-existing index
        let result = proof_db.update_task_entry(0, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry by index
        let idx = proof_db
            .insert_new_task_entry(txid, txentry.clone())
            .unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.push(3u8);

        proof_db
            .update_task_entry(idx, updated_txentry.clone())
            .unwrap();

        let stored_entry = proof_db.get_task_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_get_txentry_by_idx() {
        let proof_db = setup_db();

        // Test non-existing entry
        let result = proof_db.get_task_entry(0);
        assert!(result.is_err());

        let (txid, txentry) = generate_l1_task_entry();

        let idx = proof_db
            .insert_new_task_entry(txid, txentry.clone())
            .unwrap();

        let stored_entry = proof_db.get_task_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_get_next_txidx() {
        let proof_db = setup_db();

        let next_txidx = proof_db.get_next_task_idx().unwrap();
        assert_eq!(next_txidx, 0, "The next txidx is 0 in the beginning");

        let (txid, txentry) = generate_l1_task_entry();

        let idx = proof_db
            .insert_new_task_entry(txid, txentry.clone())
            .unwrap();

        let next_txidx = proof_db.get_next_task_idx().unwrap();

        assert_eq!(next_txidx, idx + 1);
    }
}
