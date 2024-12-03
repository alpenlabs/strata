use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt, TransactionRetry};
use strata_db::{errors::DbError, traits::ProverTaskDatabase, DbResult};
use strata_primitives::proof::ProofId;
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

impl ProverTaskDatabase for ProofDb {
    fn insert_proof(&self, proof_id: ProofId, proof: Proof) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProofSchema>(&proof_id)?.is_some() {
                    return Err(DbError::EntryAlreadyExists);
                }

                tx.put::<ProofSchema>(&proof_id, &proof)?;

                Ok(())
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_proof(&self, proof_id: ProofId) -> DbResult<Option<Proof>> {
        Ok(self.db.get::<ProofSchema>(&proof_id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance_for_prover;

    fn setup_db() -> ProofDb {
        let (db, db_ops) = get_rocksdb_tmp_instance_for_prover().unwrap();
        ProofDb::new(db, db_ops)
    }

    fn generate_proof() -> (ProofId, Proof) {
        let proof_id = ProofId::BtcBlockspace(1);
        let proof = Proof::default();
        (proof_id, proof)
    }

    #[test]
    fn test_insert_new_proof() {
        let db = setup_db();

        let (proof_id, proof) = generate_proof();

        let result = db.insert_proof(proof_id, proof.clone());
        assert!(result.is_ok(), "Proof should be inserted successfully");

        let stored_proof = db.get_proof(proof_id).unwrap();
        assert_eq!(stored_proof, Some(proof));
    }

    #[test]
    fn test_insert_duplicate_proof() {
        let db = setup_db();

        let (proof_id, proof) = generate_proof();

        db.insert_proof(proof_id, proof.clone()).unwrap();

        let result = db.insert_proof(proof_id, proof);
        assert!(result.is_err(), "Duplicate proof insertion should fail");
    }

    #[test]
    fn test_get_nonexistent_proof() {
        let db = setup_db();

        let proof_id = ProofId::BtcBlockspace(999);

        let stored_proof = db.get_proof(proof_id).unwrap();
        assert_eq!(stored_proof, None, "Nonexistent proof should return None");
    }
}
