use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaBatch, SchemaDBOperationsExt};
use strata_db::{errors::DbError, traits::*, DbResult};
use strata_state::state_op::WriteBatch;

use super::schemas::WriteBatchSchema;
use crate::{
    utils::{get_first_idx, get_last_idx},
    DbOpsConfig,
};

pub struct ChainstateDb {
    db: Arc<OptimisticTransactionDB>,
    _ops: DbOpsConfig,
}

impl ChainstateDb {
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, _ops: ops }
    }

    fn get_first_idx(&self) -> DbResult<Option<u64>> {
        get_first_idx::<WriteBatchSchema>(&self.db)
    }

    fn get_last_idx(&self) -> DbResult<Option<u64>> {
        get_last_idx::<WriteBatchSchema>(&self.db)
    }
}

impl ChainstateDatabase for ChainstateDb {
    fn write_genesis_state(&self, toplevel: strata_state::chain_state::Chainstate) -> DbResult<()> {
        let genesis_key = 0;

        // This should only ever be called once.
        if self.get_first_idx()?.is_some() || self.get_last_idx()?.is_some() {
            return Err(DbError::OverwriteStateUpdate(genesis_key));
        }

        let fake_wb = WriteBatch::new_replace(toplevel);
        self.db.put::<WriteBatchSchema>(&genesis_key, &fake_wb)?;

        Ok(())
    }

    fn put_write_batch(&self, idx: u64, batch: strata_state::state_op::WriteBatch) -> DbResult<()> {
        // Make sure we always have a contiguous range of batches.
        // FIXME this *could* be a race condition / TOCTOU issue, but we're only
        // going to be writing from a single thread anyways so it should be fine
        match self.get_last_idx()? {
            Some(last_idx) => {
                if idx != last_idx + 1 {
                    return Err(DbError::OooInsert("Chainstate", idx));
                }
            }
            None => return Err(DbError::NotBootstrapped),
        }

        if self.db.get::<WriteBatchSchema>(&idx)?.is_some() {
            return Err(DbError::OverwriteStateUpdate(idx));
        }

        // TODO maybe do this in a tx to make sure we don't race/TOCTOU it
        self.db.put::<WriteBatchSchema>(&idx, &batch)?;

        #[cfg(test)]
        eprintln!("db inserted index {idx}");

        Ok(())
    }

    fn get_write_batch(&self, idx: u64) -> DbResult<Option<strata_state::state_op::WriteBatch>> {
        Ok(self.db.get::<WriteBatchSchema>(&idx)?)
    }

    fn purge_entries_before(&self, before_idx: u64) -> DbResult<()> {
        let first_idx = match self.get_first_idx()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        if first_idx > before_idx {
            return Err(DbError::MissingL2State(before_idx));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in first_idx..before_idx {
            del_batch.delete::<WriteBatchSchema>(&idx)?;
        }
        self.db.write_schemas(del_batch)?;

        Ok(())
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        let last_idx = match self.get_last_idx()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        let first_idx = match self.get_first_idx()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        // In this case, we'd still be before the rollback idx.
        if last_idx < new_tip_idx {
            return Err(DbError::RevertAboveCurrent(new_tip_idx, last_idx));
        }

        // In this case, we'd have to roll back past the first idx.
        if first_idx > new_tip_idx {
            return Err(DbError::MissingL2State(new_tip_idx));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in new_tip_idx + 1..=last_idx {
            del_batch.delete::<WriteBatchSchema>(&idx)?;
        }
        self.db.write_schemas(del_batch)?;

        Ok(())
    }

    fn get_earliest_write_idx(&self) -> DbResult<u64> {
        self.get_first_idx()?.ok_or(DbError::NotBootstrapped)
    }

    fn get_last_write_idx(&self) -> DbResult<u64> {
        self.get_last_idx()?.ok_or(DbError::NotBootstrapped)
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use strata_state::{chain_state::Chainstate, state_op::WriteBatch};
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn setup_db() -> ChainstateDb {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        ChainstateDb::new(db, db_ops)
    }

    #[test]
    fn test_write_genesis_state() {
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let db = setup_db();

        let res = db.get_earliest_write_idx();
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let res = db.get_last_write_idx();
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let res = db.write_genesis_state(genesis_state.clone());
        assert!(res.is_ok());

        let res = db.get_earliest_write_idx();
        assert!(res.is_ok_and(|x| matches!(x, 0)));

        let res = db.get_last_write_idx();
        assert!(res.is_ok_and(|x| matches!(x, 0)));

        let res = db.write_genesis_state(genesis_state);
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteStateUpdate(0))));
    }

    #[test]
    fn test_write_state_update() {
        let db = setup_db();
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_replace(genesis_state.clone());

        let res = db.put_write_batch(1, batch.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        db.write_genesis_state(genesis_state).unwrap();

        let res = db.put_write_batch(1, batch.clone());
        assert!(res.is_ok());

        let res = db.put_write_batch(2, batch.clone());
        assert!(res.is_ok());

        let res = db.put_write_batch(2, batch.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteStateUpdate(2))));

        let res = db.put_write_batch(4, batch.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("Chainstate", 4))));
    }

    #[test]
    fn test_get_earliest_and_last_state_idx() {
        let db = setup_db();
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_replace(genesis_state.clone());

        db.write_genesis_state(genesis_state).unwrap();
        for i in 1..=5 {
            eprintln!("test inserting index {i}");
            assert_eq!(db.get_earliest_write_idx().unwrap(), 0);
            db.put_write_batch(i, batch.clone()).unwrap();
            assert_eq!(db.get_last_write_idx().unwrap(), i);
        }
    }

    #[test]
    fn test_purge() {
        let db = setup_db();
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_replace(genesis_state.clone());

        db.write_genesis_state(genesis_state).unwrap();
        for i in 1..=5 {
            eprintln!("test inserting index {i}");
            assert_eq!(db.get_earliest_write_idx().unwrap(), 0);
            db.put_write_batch(i, batch.clone()).unwrap();
            assert_eq!(db.get_last_write_idx().unwrap(), i);
        }

        db.purge_entries_before(3).unwrap();
        // Ensure that calling the purge again does not fail
        db.purge_entries_before(3).unwrap();

        assert_eq!(db.get_earliest_write_idx().unwrap(), 3);
        assert_eq!(db.get_last_write_idx().unwrap(), 5);

        for i in 0..3 {
            assert!(db.get_write_batch(i).unwrap().is_none());
        }

        for i in 3..=5 {
            assert!(db.get_write_batch(i).unwrap().is_some());
        }

        let res = db.purge_entries_before(2);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(2))));

        let res = db.purge_entries_before(1);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(1))));
    }

    #[test]
    fn test_rollback() {
        let db = setup_db();
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_replace(genesis_state.clone());

        db.write_genesis_state(genesis_state).unwrap();
        for i in 1..=5 {
            db.put_write_batch(i, batch.clone()).unwrap();
        }

        db.rollback_writes_to(3).unwrap();
        // Ensures that calling the rollback again does not fail
        db.rollback_writes_to(3).unwrap();

        for i in 4..=5 {
            assert!(db.get_write_batch(i).unwrap().is_none());
        }

        // For genesis there is no BatchWrites
        for i in 1..=3 {
            assert!(db.get_write_batch(i).unwrap().is_some());
        }

        assert_eq!(db.get_earliest_write_idx().unwrap(), 0);
        assert_eq!(db.get_last_write_idx().unwrap(), 3);

        let res = db.rollback_writes_to(5);
        assert!(res.is_err_and(|x| matches!(x, DbError::RevertAboveCurrent(5, 3))));

        let res = db.rollback_writes_to(4);
        assert!(res.is_err_and(|x| matches!(x, DbError::RevertAboveCurrent(4, 3))));

        let res = db.rollback_writes_to(3);
        assert!(res.is_ok());

        db.rollback_writes_to(2).unwrap();
        assert_eq!(db.get_earliest_write_idx().unwrap(), 0);
        assert_eq!(db.get_last_write_idx().unwrap(), 2);
    }

    #[test]
    fn test_purge_and_rollback() {
        let db = setup_db();
        let genesis_state: Chainstate = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_replace(genesis_state.clone());

        db.write_genesis_state(genesis_state).unwrap();
        for i in 1..=5 {
            db.put_write_batch(i, batch.clone()).unwrap();
        }

        db.purge_entries_before(3).unwrap();

        let res = db.rollback_writes_to(3);
        assert!(res.is_ok());

        let res = db.rollback_writes_to(2);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(2))));
    }
}
