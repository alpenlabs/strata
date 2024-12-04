use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt, TransactionRetry};
use strata_db::{
    errors::DbError,
    traits::{ProofDatabase, ProverDatabase},
    DbResult,
};
use strata_primitives::proof::ProofKey;
use strata_zkvm::Proof;

use super::schemas::ProofSchema;
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

impl ProofDatabase for ProofDb {
    fn put_proof(&self, proof_key: ProofKey, proof: Proof) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProofSchema>(&proof_key)?.is_some() {
                    return Err(DbError::EntryAlreadyExists);
                }

                tx.put::<ProofSchema>(&proof_key, &proof)?;

                Ok(())
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_proof(&self, proof_key: ProofKey) -> DbResult<Option<Proof>> {
        Ok(self.db.get::<ProofSchema>(&proof_key)?)
    }

    fn del_proof(&self, proof_key: ProofKey) -> DbResult<bool> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProofSchema>(&proof_key)?.is_none() {
                    return Ok(false);
                }
                tx.delete::<ProofSchema>(&proof_key)?;

                Ok::<_, anyhow::Error>(true)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }
}

pub struct ProverDB {
    db: Arc<ProofDb>,
}

impl ProverDB {
    pub fn new(db: Arc<ProofDb>) -> Self {
        Self { db }
    }
}

impl ProverDatabase for ProverDB {
    type ProofDB = ProofDb;

    fn proof_db(&self) -> &Arc<Self::ProofDB> {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use strata_state::l1::L1BlockId;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance_for_prover;

    fn setup_db() -> ProofDb {
        let (db, db_ops) = get_rocksdb_tmp_instance_for_prover().unwrap();
        ProofDb::new(db, db_ops)
    }

    fn generate_proof() -> (ProofKey, Proof) {
        let proof_key = ProofKey::BtcBlockspace(L1BlockId::default());
        let proof = Proof::default();
        (proof_key, proof)
    }

    #[test]
    fn test_insert_new_proof() {
        let db = setup_db();

        let (proof_key, proof) = generate_proof();

        let result = db.put_proof(proof_key, proof.clone());
        assert!(result.is_ok(), "Proof should be inserted successfully");

        let stored_proof = db.get_proof(proof_key).unwrap();
        assert_eq!(stored_proof, Some(proof));
    }

    #[test]
    fn test_insert_duplicate_proof() {
        let db = setup_db();

        let (proof_key, proof) = generate_proof();

        db.put_proof(proof_key, proof.clone()).unwrap();

        let result = db.put_proof(proof_key, proof);
        assert!(result.is_err(), "Duplicate proof insertion should fail");
    }

    #[test]
    fn test_get_nonexistent_proof() {
        let db = setup_db();

        let (proof_key, proof) = generate_proof();
        db.put_proof(proof_key, proof.clone()).unwrap();

        let res = db.del_proof(proof_key);
        assert!(matches!(res, Ok(true)));

        let res = db.del_proof(proof_key);
        assert!(matches!(res, Ok(false)));

        let stored_proof = db.get_proof(proof_key).unwrap();
        assert_eq!(stored_proof, None, "Nonexistent proof should return None");
    }
}
