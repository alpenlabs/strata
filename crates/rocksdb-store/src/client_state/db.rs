use std::sync::Arc;

use rockbound::Schema;
use rockbound::SchemaDBOperationsExt;

use alpen_express_db::{errors::*, traits::*, DbResult};
use alpen_express_state::operation::*;

use crate::OptimisticDb;

use super::schemas::{ClientStateSchema, ClientUpdateOutputSchema};

pub struct ClientStateDb {
    db: Arc<OptimisticDb>,
}

impl ClientStateDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<OptimisticDb>) -> Self {
        Self { db }
    }

    fn get_last_idx<T>(&self) -> DbResult<Option<u64>>
    where
        T: Schema<Key = u64>,
    {
        let mut iterator = self.db.db.iter::<T>()?;
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

impl ClientStateStore for ClientStateDb {
    fn write_client_update_output(&self, idx: u64, output: ClientUpdateOutput) -> DbResult<()> {
        let expected_idx = match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(last_idx) => last_idx + 1,
            None => 1,
        };
        if idx != expected_idx {
            return Err(DbError::OooInsert("consensus_store", idx));
        }
        self.db.db.put::<ClientUpdateOutputSchema>(&idx, &output)?;
        Ok(())
    }

    fn write_client_state_checkpoint(
        &self,
        idx: u64,
        state: alpen_express_state::client_state::ClientState,
    ) -> DbResult<()> {
        // FIXME this should probably be a transaction
        if self.db.db.get::<ClientStateSchema>(&idx)?.is_some() {
            return Err(DbError::OverwriteConsensusCheckpoint(idx));
        }
        self.db.db.put::<ClientStateSchema>(&idx, &state)?;
        Ok(())
    }
}

impl ClientStateProvider for ClientStateDb {
    fn get_last_write_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ClientUpdateOutputSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_client_state_writes(&self, idx: u64) -> DbResult<Option<Vec<ClientStateWrite>>> {
        let output = self.db.db.get::<ClientUpdateOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.writes().to_owned())),
            None => Ok(None),
        }
    }

    fn get_client_update_actions(&self, idx: u64) -> DbResult<Option<Vec<SyncAction>>> {
        let output = self.db.db.get::<ClientUpdateOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.actions().to_owned())),
            None => Ok(None),
        }
    }

    fn get_last_checkpoint_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ClientStateSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_prev_checkpoint_at(&self, idx: u64) -> DbResult<u64> {
        let mut iterator = self.db.db.iter::<ClientStateSchema>()?;
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

    fn get_state_checkpoint(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_express_state::client_state::ClientState>> {
        Ok(self.db.db.get::<ClientStateSchema>(&idx)?)
    }
}

#[cfg(test)]
mod tests {
    use alpen_express_state::client_state::ClientState;
    use alpen_test_utils::*;

    use crate::test_utils::get_rocksdb_tmp_instance;

    use super::*;

    fn setup_db() -> ClientStateDb {
        let db = get_rocksdb_tmp_instance().unwrap();
        ClientStateDb::new(db)
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
