use std::sync::Arc;

use anyhow::Context;
use rockbound::{OptimisticTransactionDB, Schema, SchemaDBOperationsExt};
use strata_db::{errors::*, traits::*, DbResult};
use strata_state::operation::*;

use super::schemas::ClientUpdateOutputSchema;
use crate::DbOpsConfig;

pub struct ClientStateDb {
    db: Arc<OptimisticTransactionDB>,
    _ops: DbOpsConfig,
}

impl ClientStateDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, _ops: ops }
    }

    fn get_last_idx<T>(&self) -> DbResult<Option<u64>>
    where
        T: Schema<Key = u64>,
    {
        let mut iterator = self.db.iter::<T>()?;
        iterator.seek_to_last();
        match iterator.rev().next() {
            Some(res) => {
                let (tip, _) = res?.into_tuple();
                Ok(Some(tip))
            }
            None => Ok(None),
        }
    }
}

impl ClientStateDatabase for ClientStateDb {
    fn put_client_update(&self, idx: u64, output: ClientUpdateOutput) -> DbResult<()> {
        let expected_idx = match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(last_idx) => last_idx + 1,

            // We don't have a separate way to insert the init client state, so
            // we special case this here.
            None => 0,
        };

        if idx != expected_idx {
            return Err(DbError::OooInsert("consensus_store", idx));
        }

        self.db.put::<ClientUpdateOutputSchema>(&idx, &output)?;
        Ok(())
    }

    fn get_client_update(&self, idx: u64) -> DbResult<Option<ClientUpdateOutput>> {
        Ok(self.db.get::<ClientUpdateOutputSchema>(&idx)?)
    }

    fn get_last_state_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::*;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn setup_db() -> ClientStateDb {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        ClientStateDb::new(db, db_ops)
    }

    #[test]
    fn test_get_last_idx() {
        let db = setup_db();
        let idx = db.get_last_idx::<ClientUpdateOutputSchema>().unwrap();
        assert_eq!(idx, None);
    }

    #[test]
    fn test_write_consensus_output() {
        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();
        let db = setup_db();

        let res = db.put_client_update(2, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 2))));

        let res = db.put_client_update(1, output.clone());
        assert!(res.is_ok());

        let res = db.put_client_update(1, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 1))));

        let res = db.put_client_update(3, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 3))));
    }

    #[test]
    fn test_get_last_write_idx() {
        let db = setup_db();

        let idx = db.get_last_state_idx();
        assert!(idx.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();
        let _ = db.put_client_update(1, output.clone());

        let idx = db.get_last_state_idx();
        assert!(idx.is_ok_and(|x| matches!(x, 1)));
    }

    #[test]
    fn test_get_consensus_state() {
        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();

        let db = setup_db();
        let _ = db.put_client_update(1, output.clone());

        let update = db.get_client_update(1).unwrap().unwrap();
        let state = update.state();
        assert_eq!(state, output.state());
    }

    #[test]
    fn test_get_consensus_actions() {
        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();

        let db = setup_db();
        let _ = db.put_client_update(1, output.clone());

        let update = db.get_client_update(1).unwrap().unwrap();
        let actions = update.actions();
        assert_eq!(actions, output.actions());
    }
}
