use std::sync::Arc;

use rockbound::{Schema, DB};

use alpen_vertex_state::operation::*;

use super::schemas::{ConsensusOutputSchema, ConsensusStateSchema};
use crate::errors::*;
use crate::traits::*;

pub struct ConsensusStateDb {
    db: Arc<DB>,
}

impl ConsensusStateDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
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

impl ConsensusStateStore for ConsensusStateDb {
    fn write_consensus_output(&self, idx: u64, output: ConsensusOutput) -> DbResult<()> {
        let expected_idx = match self.get_last_idx::<ConsensusOutputSchema>()? {
            Some(last_idx) => last_idx + 1,
            None => 1,
        };
        if idx != expected_idx {
            return Err(DbError::OooInsert("consensus_store", idx));
        }
        self.db.put::<ConsensusOutputSchema>(&idx, &output)?;
        Ok(())
    }

    fn write_consensus_checkpoint(
        &self,
        idx: u64,
        state: alpen_vertex_state::consensus::ConsensusState,
    ) -> DbResult<()> {
        // FIXME this should probably be a transaction
        if self.db.get::<ConsensusStateSchema>(&idx)?.is_some() {
            return Err(DbError::OverwriteConsensusCheckpoint(idx));
        }
        self.db.put::<ConsensusStateSchema>(&idx, &state)?;
        Ok(())
    }
}

impl ConsensusStateProvider for ConsensusStateDb {
    fn get_last_write_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ConsensusOutputSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_consensus_writes(&self, idx: u64) -> DbResult<Option<Vec<ConsensusWrite>>> {
        let output = self.db.get::<ConsensusOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.writes().to_owned())),
            None => Ok(None),
        }
    }

    fn get_consensus_actions(&self, idx: u64) -> DbResult<Option<Vec<SyncAction>>> {
        let output = self.db.get::<ConsensusOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.actions().to_owned())),
            None => Ok(None),
        }
    }

    fn get_last_checkpoint_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ConsensusStateSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_prev_checkpoint_at(&self, idx: u64) -> DbResult<u64> {
        let mut iterator = self.db.iter::<ConsensusStateSchema>()?;
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
    use std::path::Path;

    use arbitrary::{Arbitrary, Unstructured};
    use rockbound::schema::ColumnFamilyName;
    use rocksdb::Options;
    use tempfile::TempDir;

    use alpen_vertex_state::consensus::ConsensusState;

    use super::*;
    use crate::STORE_COLUMN_FAMILIES;

    const DB_NAME: &str = "consensus_state_db";

    fn get_new_db(path: &Path) -> anyhow::Result<Arc<DB>> {
        // TODO: add other options as appropriate.
        let mut db_opts = Options::default();
        db_opts.create_missing_column_families(true);
        db_opts.create_if_missing(true);
        DB::open(
            path,
            DB_NAME,
            STORE_COLUMN_FAMILIES
                .iter()
                .cloned()
                .collect::<Vec<ColumnFamilyName>>(),
            &db_opts,
        )
        .map(Arc::new)
    }

    fn setup_db() -> ConsensusStateDb {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = get_new_db(&temp_dir.into_path()).unwrap();
        ConsensusStateDb::new(db)
    }

    fn generate_arbitrary<'a, T: Arbitrary<'a> + Clone>(bytes: &'a [u8]) -> T {
        // Create an Unstructured instance and generate the arbitrary value
        let mut u = Unstructured::new(bytes);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }

    #[test]
    fn test_get_last_idx() {
        let db = setup_db();
        let idx = db.get_last_idx::<ConsensusOutputSchema>().unwrap();
        assert_eq!(idx, None);
    }

    #[test]
    fn test_write_consensus_output() {
        let output: ConsensusOutput = generate_arbitrary(&[1, 2, 3]);
        let db = setup_db();

        let res = db.write_consensus_output(2, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 2))));

        let res = db.write_consensus_output(1, output.clone());
        assert!(res.is_ok());

        let res = db.write_consensus_output(1, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 1))));

        let res = db.write_consensus_output(3, output.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OooInsert("consensus_store", 3))));
    }

    #[test]
    fn test_get_last_write_idx() {
        let db = setup_db();

        let idx = db.get_last_write_idx();
        assert!(idx.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        let output: ConsensusOutput = generate_arbitrary(&[1, 2, 3]);
        let _ = db.write_consensus_output(1, output.clone());

        let idx = db.get_last_write_idx();
        assert!(idx.is_ok_and(|x| matches!(x, 1)));
    }

    #[test]
    fn test_get_consensus_writes() {
        let output: ConsensusOutput = generate_arbitrary(&[1, 2, 3]);

        let db = setup_db();
        let _ = db.write_consensus_output(1, output.clone());

        let writes = db.get_consensus_writes(1).unwrap().unwrap();
        assert_eq!(&writes, output.writes());
    }

    #[test]
    fn test_get_consensus_actions() {
        let output: ConsensusOutput = generate_arbitrary(&[1, 2, 3]);

        let db = setup_db();
        let _ = db.write_consensus_output(1, output.clone());

        let actions = db.get_consensus_actions(1).unwrap().unwrap();
        assert_eq!(&actions, output.actions());
    }

    #[test]
    fn test_write_consensus_checkpoint() {
        let state: ConsensusState = generate_arbitrary(&[1, 2, 3]);
        let db = setup_db();

        let _ = db.write_consensus_checkpoint(3, state.clone());

        let idx = db.get_last_checkpoint_idx().unwrap();
        assert_eq!(idx, 3);

        let res = db.write_consensus_checkpoint(3, state.clone());
        assert!(res.is_err_and(|x| matches!(x, DbError::OverwriteConsensusCheckpoint(3))));

        // TODO: verify if it is possible to write checkpoint in any order
        let res = db.write_consensus_checkpoint(1, state);
        assert!(res.is_ok());

        // Note: The ordering is managed by rocksdb. So might be alright..
        let idx = db.get_last_checkpoint_idx().unwrap();
        assert_eq!(idx, 3);
    }

    #[test]
    fn test_get_previous_checkpoint_at() {
        let state: ConsensusState = generate_arbitrary(&[1, 2, 3]);

        let db = setup_db();

        let res = db.get_prev_checkpoint_at(1);
        assert!(res.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));

        // Add a checkpoint
        _ = db.write_consensus_checkpoint(1, state.clone());

        let res = db.get_prev_checkpoint_at(1);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(2);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        let res = db.get_prev_checkpoint_at(100);
        assert!(res.is_ok_and(|x| matches!(x, 1)));

        // Add a new checkpoint
        _ = db.write_consensus_checkpoint(5, state.clone());

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
