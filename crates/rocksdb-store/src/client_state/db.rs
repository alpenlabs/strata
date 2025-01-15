use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, Schema, SchemaDBOperationsExt};
use strata_db::{errors::*, traits::*, DbResult};
use strata_state::operation::*;

use super::schemas::{ClientStateSchema, ClientUpdateOutputSchema};
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
    fn write_client_update(&self, idx: u64, output: ClientUpdateOutput) -> DbResult<()> {
        let expected_idx = match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(last_idx) => last_idx + 1,
            None => 1,
        };
        if idx != expected_idx {
            return Err(DbError::OooInsert("consensus_store", idx));
        }
        self.db.put::<ClientUpdateOutputSchema>(&idx, &output)?;
        Ok(())
    }

    fn get_last_update_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_client_update(&self, idx: u64) -> DbResult<Option<ClientUpdateOutput>> {
        Ok(self.db.get::<ClientUpdateOutputSchema>(&idx)?)
    }

    fn get_prev_update_at(&self, idx: u64) -> DbResult<u64> {
        let mut iterator = self.db.iter::<ClientStateSchema>()?;
        iterator.seek_to_last();
        let rev_iterator = iterator.rev();

        for res in rev_iterator {
            match res {
                Ok(item) => {
                    let (tip, _) = item.into_tuple();
                    if tip <= idx {
                        return Ok(tip);
                    }
                }
                Err(e) => return Err(DbError::Other(e.to_string())),
            }
        }

        Err(DbError::NotBootstrapped)
    }
}

#[cfg(test)]
mod tests {
    use strata_state::client_state::ClientState;
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

        let res = db.write_client_update_output(2, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 2))));

        let res = db.write_client_update_output(1, output.clone());
        assert!(res.is_ok());

        let res = db.write_client_update_output(1, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 1))));

        let res = db.write_client_update_output(3, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 3))));
    }

    #[test]
    fn test_get_last_write_idx() {
        let db = setup_db();

        let idx = db.get_last_write_idx();
        assert!(idx.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();
        let _ = db.write_client_update_output(1, output.clone());

        let idx = db.get_last_write_idx();
        assert!(idx.is_ok_and(|x| matches!(x, 1)));
    }

    #[test]
    fn test_get_consensus_writes() {
        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();

        let db = setup_db();
        let _ = db.write_client_update_output(1, output.clone());

        let writes = db.get_client_state_writes(1).unwrap().unwrap();
        assert_eq!(&writes, output.writes());
    }

    #[test]
    fn test_get_consensus_actions() {
        let output: ClientUpdateOutput = ArbitraryGenerator::new().generate();

        let db = setup_db();
        let _ = db.write_client_update_output(1, output.clone());

        let actions = db.get_client_update_actions(1).unwrap().unwrap();
        assert_eq!(&actions, output.actions());
    }

    #[test]
    fn test_write_consensus_checkpoint() {
        let state: ClientState = ArbitraryGenerator::new().generate();
        let db = setup_db();

        let _ = db.write_client_state_checkpoint(3, state.clone());

        let idx = db.get_last_checkpoint_idx().unwrap();
        assert_eq!(idx, 3);

        let res = db.write_client_state_checkpoint(3, state.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteConsensusCheckpoint(3))));

        // TODO: verify if it is possible to write checkpoint in any order
        let res = db.write_client_state_checkpoint(1, state);
        assert!(res.is_ok());

        // Note: The ordering is managed by rocksdb. So might be alright..
        let idx = db.get_last_checkpoint_idx().unwrap();
        assert_eq!(idx, 3);
    }

    #[test]
    fn test_get_previous_checkpoint_at() {
        let state: ClientState = ArbitraryGenerator::new().generate();

        let db = setup_db();

        let res = db.get_prev_checkpoint_at(1);
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        // Add a checkpoint
        _ = db.write_client_state_checkpoint(1, state.clone());

        let res = db.get_prev_checkpoint_at(1);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(2);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(100);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        // Add a new checkpoint
        _ = db.write_client_state_checkpoint(5, state.clone());

        let res = db.get_prev_checkpoint_at(1);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(2);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(5);
        assert!(res.is_ok_and(|x| matches!(x, 5)));

        let res = db.get_prev_checkpoint_at(100);
        assert!(res.is_ok_and(|x| matches!(x, 5)));
    }
}
