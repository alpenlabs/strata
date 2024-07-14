use std::sync::Arc;

use alpen_vertex_state::state_op;
use rockbound::SchemaBatch;
use rockbound::DB;

use super::schemas::{ChainStateSchema, WriteBatchSchema};
use crate::errors::*;
use crate::traits::*;
use crate::utils::{get_first_idx, get_last_idx};

pub struct ChainStateDb {
    db: Arc<DB>,
}

impl ChainStateDb {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn get_first_idx(&self) -> DbResult<Option<u64>> {
        get_first_idx::<ChainStateSchema>(self.db.clone())
    }

    fn get_last_idx(&self) -> DbResult<Option<u64>> {
        get_last_idx::<ChainStateSchema>(self.db.clone())
    }
}

impl ChainstateProvider for ChainStateDb {
    fn get_earliest_state_idx(&self) -> DbResult<u64> {
        match self.get_first_idx()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_last_state_idx(&self) -> DbResult<u64> {
        match self.get_last_idx()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_writes_at(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::state_op::WriteBatch>> {
        Ok(self.db.get::<WriteBatchSchema>(&idx)?)
    }

    // TODO: define what toplevel means more clearly
    fn get_toplevel_state(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::chain_state::ChainState>> {
        Ok(self.db.get::<ChainStateSchema>(&idx)?)
    }
}

impl ChainstateStore for ChainStateDb {
    fn write_genesis_state(
        &self,
        toplevel: &alpen_vertex_state::chain_state::ChainState,
    ) -> DbResult<()> {
        let genesis_key = 0;
        if self.get_first_idx()?.is_some() || self.get_last_idx()?.is_some() {
            return Err(DbError::OverwriteStateUpdate(genesis_key));
        }
        self.db.put::<ChainStateSchema>(&genesis_key, toplevel)?;
        Ok(())
    }

    fn write_state_update(
        &self,
        idx: u64,
        batch: &alpen_vertex_state::state_op::WriteBatch,
    ) -> DbResult<()> {
        if let Some(_) = self.db.get::<WriteBatchSchema>(&idx)? {
            return Err(DbError::OverwriteStateUpdate(idx));
        }

        let pre_state_idx = idx - 1;
        let pre_state = match self.db.get::<ChainStateSchema>(&pre_state_idx)? {
            Some(state) => state,
            None => return Err(DbError::OooInsert("ChainState", idx)),
        };
        let post_state = state_op::apply_write_batch_to_chainstate(pre_state, batch);

        let mut write_batch = SchemaBatch::new();
        write_batch.put::<WriteBatchSchema>(&idx, batch)?;
        write_batch.put::<ChainStateSchema>(&idx, &post_state)?;
        self.db.write_schemas(write_batch)?;

        Ok(())
    }

    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()> {
        let first_idx = match self.get_first_idx()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        if first_idx > before_idx {
            return Err(DbError::MissingL2State(before_idx));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in first_idx..before_idx {
            del_batch.delete::<ChainStateSchema>(&idx)?;
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

        if last_idx < new_tip_idx {
            return Err(DbError::RevertAboveCurrent(new_tip_idx, last_idx));
        }

        if first_idx > new_tip_idx {
            return Err(DbError::MissingL2State(new_tip_idx));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in new_tip_idx + 1..=last_idx {
            del_batch.delete::<ChainStateSchema>(&idx)?;
            del_batch.delete::<ChainStateSchema>(&idx)?;
        }
        self.db.write_schemas(del_batch)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::{get_rocksdb_tmp_instance, ArbitraryGenerator};
    use alpen_vertex_state::chain_state::ChainState;
    use state_op::WriteBatch;

    use super::*;

    fn setup_db() -> ChainStateDb {
        let db = get_rocksdb_tmp_instance().unwrap();
        ChainStateDb::new(db)
    }

    #[test]
    fn test_write_genesis_state() {
        let genesis_state: ChainState = ArbitraryGenerator::new().generate();
        let db = setup_db();

        let res = db.get_earliest_state_idx();
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let res = db.get_last_state_idx();
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let res = db.write_genesis_state(&genesis_state);
        assert!(res.is_ok());

        let res = db.get_earliest_state_idx();
        assert!(res.is_ok_and(|x| matches!(x, 0)));

        let res = db.get_last_state_idx();
        assert!(res.is_ok_and(|x| matches!(x, 0)));

        let res = db.write_genesis_state(&genesis_state);
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteStateUpdate(0))));
    }

    #[test]
    fn test_write_state_update() {
        let db = setup_db();
        let batch = WriteBatch::new_empty();

        let res = db.write_state_update(1, &batch);
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("ChainState", 1))));

        let genesis_state: ChainState = ArbitraryGenerator::new().generate();
        db.write_genesis_state(&genesis_state).unwrap();

        let res = db.write_state_update(1, &batch);
        assert!(res.is_ok());

        let res = db.write_state_update(2, &batch);
        assert!(res.is_ok());

        let res = db.write_state_update(2, &batch);
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteStateUpdate(2))));

        let res = db.write_state_update(4, &batch);
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("ChainState", 4))));
    }

    #[test]
    fn test_purge() {
        let db = setup_db();
        let genesis_state: ChainState = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_empty();

        db.write_genesis_state(&genesis_state).unwrap();
        for i in 1..=5 {
            assert_eq!(db.get_earliest_state_idx().unwrap(), 0);
            db.write_state_update(i, &batch).unwrap();
            assert_eq!(db.get_last_state_idx().unwrap(), i);
        }

        db.purge_historical_state_before(3).unwrap();
        db.purge_historical_state_before(3).unwrap();

        assert_eq!(db.get_earliest_state_idx().unwrap(), 3);
        assert_eq!(db.get_last_state_idx().unwrap(), 5);

        let res = db.purge_historical_state_before(2);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(2))));

        let res = db.purge_historical_state_before(1);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(1))));
    }

    #[test]
    fn test_rollback() {
        let db = setup_db();
        let genesis_state: ChainState = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_empty();

        db.write_genesis_state(&genesis_state).unwrap();
        for i in 1..=5 {
            db.write_state_update(i, &batch).unwrap();
        }

        db.rollback_writes_to(3).unwrap();
        db.rollback_writes_to(3).unwrap();

        assert_eq!(db.get_earliest_state_idx().unwrap(), 0);
        assert_eq!(db.get_last_state_idx().unwrap(), 3);

        let res = db.rollback_writes_to(5);
        assert!(res.is_err_and(|x| matches!(x, DbError::RevertAboveCurrent(5, 3))));

        let res = db.rollback_writes_to(4);
        assert!(res.is_err_and(|x| matches!(x, DbError::RevertAboveCurrent(4, 3))));

        let res = db.rollback_writes_to(3);
        assert!(res.is_ok());

        db.rollback_writes_to(2).unwrap();
        assert_eq!(db.get_earliest_state_idx().unwrap(), 0);
        assert_eq!(db.get_last_state_idx().unwrap(), 2);
    }

    #[test]
    fn test_purge_and_rollback() {
        let db = setup_db();
        let genesis_state: ChainState = ArbitraryGenerator::new().generate();
        let batch = WriteBatch::new_empty();

        db.write_genesis_state(&genesis_state).unwrap();
        for i in 1..=5 {
            db.write_state_update(i, &batch).unwrap();
        }

        db.purge_historical_state_before(3).unwrap();

        let res = db.rollback_writes_to(3);
        assert!(res.is_ok());

        let res = db.rollback_writes_to(2);
        assert!(res.is_err_and(|x| matches!(x, DbError::MissingL2State(2))));
    }
}
