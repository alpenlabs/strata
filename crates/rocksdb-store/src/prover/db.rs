use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaDBOperationsExt, TransactionRetry};
use strata_db::{
    errors::DbError,
    traits::{ProofDatabase, ProverDatabase},
    DbResult,
};
use strata_primitives::proof::{ProofId, ProofKey};
use strata_zkvm::ProofReceipt;

use super::schemas::{ProofDepsSchema, ProofSchema};
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
    fn put_proof(&self, proof_key: ProofKey, proof: ProofReceipt) -> DbResult<()> {
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

    fn get_proof(&self, proof_key: ProofKey) -> DbResult<Option<ProofReceipt>> {
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

    fn put_proof_deps(&self, proof_id: ProofId, deps: Vec<ProofId>) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProofDepsSchema>(&proof_id)?.is_some() {
                    return Err(DbError::EntryAlreadyExists);
                }

                tx.put::<ProofDepsSchema>(&proof_id, &deps)?;

                Ok(())
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn get_proof_deps(&self, proof_id: ProofId) -> DbResult<Option<Vec<ProofId>>> {
        Ok(self.db.get::<ProofDepsSchema>(&proof_id)?)
    }

    fn del_proof_deps(&self, proof_id: ProofId) -> DbResult<bool> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<ProofDepsSchema>(&proof_id)?.is_none() {
                    return Ok(false);
                }
                tx.delete::<ProofDepsSchema>(&proof_id)?;

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
    use strata_primitives::{
        buf::Buf32,
        proof::{ProofId, ProofZkVm},
    };
    use strata_state::l1::L1BlockId;
    use strata_zkvm::{Proof, PublicValues};

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance_for_prover;

    fn setup_db() -> ProofDb {
        let (db, db_ops) = get_rocksdb_tmp_instance_for_prover().unwrap();
        ProofDb::new(db, db_ops)
    }

    fn generate_proof() -> (ProofKey, ProofReceipt) {
        let proof_id = ProofId::BtcBlockspace(L1BlockId::default());
        let host = ProofZkVm::Native;
        let proof_key = ProofKey::new(proof_id, host);
        let proof = Proof::default();
        let public_values = PublicValues::default();
        let proof_receipt = ProofReceipt::new(proof, public_values);
        (proof_key, proof_receipt)
    }

    fn generate_proof_id_with_deps() -> (ProofId, Vec<ProofId>) {
        let l1_blkid_1: L1BlockId = Buf32::from([1u8; 32]).into();
        let l1_blkid_2: L1BlockId = Buf32::from([2u8; 32]).into();
        let proof_id = ProofId::L1Batch(l1_blkid_1, l1_blkid_2);
        let deps = vec![
            ProofId::BtcBlockspace(l1_blkid_1),
            ProofId::BtcBlockspace(l1_blkid_2),
        ];
        (proof_id, deps)
    }

    #[test]
    fn test_insert_new_proof() {
        let db = setup_db();

        let (proof_key, proof) = generate_proof();

        let result = db.put_proof(proof_key, proof.clone());
        assert!(
            result.is_ok(),
            "ProofReceipt should be inserted successfully"
        );

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

    #[test]
    fn test_insert_new_deps() {
        let db = setup_db();

        let (proof_id, deps) = generate_proof_id_with_deps();

        let result = db.put_proof_deps(proof_id, deps.clone());
        assert!(
            result.is_ok(),
            "ProofReceipt should be inserted successfully"
        );

        let stored_deps = db.get_proof_deps(proof_id).unwrap();
        assert_eq!(stored_deps, Some(deps));
    }

    #[test]
    fn test_insert_duplicate_proof_deps() {
        let db = setup_db();

        let (proof_id, deps) = generate_proof_id_with_deps();

        db.put_proof_deps(proof_id, deps.clone()).unwrap();

        let result = db.put_proof_deps(proof_id, deps);
        assert!(
            result.is_err(),
            "Duplicate proof deps insertion should fail"
        );
    }

    #[test]
    fn test_get_nonexistent_proof_deps() {
        let db = setup_db();

        let (proof_id, deps) = generate_proof_id_with_deps();
        db.put_proof_deps(proof_id, deps.clone()).unwrap();

        let res = db.del_proof_deps(proof_id);
        assert!(matches!(res, Ok(true)));

        let res = db.del_proof_deps(proof_id);
        assert!(matches!(res, Ok(false)));

        let stored_proof = db.get_proof_deps(proof_id).unwrap();
        assert_eq!(
            stored_proof, None,
            "Nonexistent proof deps should return None"
        );
    }
}
