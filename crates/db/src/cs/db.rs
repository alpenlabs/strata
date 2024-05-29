use anyhow::anyhow;
use rockbound::{Schema, DB};
use rocksdb::Options;

use std::{path::Path};

use crate::{
    errors::DbError,
    traits::{ConsensusStateProvider, ConsensusStateStore},
};

use super::schemas::{ConsensusOutputSchema, ConsensusStateSchema};

const DB_NAME: &str = "cs_db";

pub struct CsDb {
    db: DB,
}

fn get_db_opts() -> Options {
    // TODO: add other options as appropriate.
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);
    db_opts
}

impl CsDb {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let db_opts = get_db_opts();
        let column_families = vec![
            crate::cs::schemas::ConsensusOutputSchema::COLUMN_FAMILY_NAME,
            crate::cs::schemas::ConsensusStateSchema::COLUMN_FAMILY_NAME,
        ];
        let store = Self {
            db: rockbound::DB::open(path, DB_NAME, column_families, &db_opts)?,
        };
        Ok(store)
    }

    fn get_last_idx<T>(&self) -> crate::DbResult<Option<u64>>
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

impl ConsensusStateStore for CsDb {
    fn write_consensus_output(
        &self,
        idx: u64,
        output: crate::traits::ConsensusOutput,
    ) -> crate::DbResult<()> {
        let expected_idx = match self.get_last_idx::<ConsensusOutputSchema>()? {
            Some(last_idx) => last_idx + 1,
            None => 1,
        };
        if idx != expected_idx {
            return Err(DbError::OooInsert("Consensus store", idx));
        }
        self.db.put::<ConsensusOutputSchema>(&idx, &output)?;
        Ok(())
    }

    fn write_consensus_checkpoint(
        &self,
        idx: u64,
        state: alpen_vertex_state::consensus::ConsensusState,
    ) -> crate::DbResult<()> {
        if self.db.get::<ConsensusStateSchema>(&idx)?.is_some() {
            return Err(DbError::DuplicateKey(idx));
        }
        self.db.put::<ConsensusStateSchema>(&idx, &state)?;
        Ok(())
    }
}

impl ConsensusStateProvider for CsDb {
    fn get_last_write_idx(&self) -> crate::DbResult<u64> {
        match self.get_last_idx::<ConsensusOutputSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_consensus_writes(
        &self,
        idx: u64,
    ) -> crate::DbResult<Option<Vec<alpen_vertex_state::consensus::ConsensusWrite>>> {
        let output = self.db.get::<ConsensusOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.writes().to_owned())),
            None => Ok(None),
        }
    }

    fn get_consensus_actions(
        &self,
        idx: u64,
    ) -> crate::DbResult<Option<Vec<alpen_vertex_state::sync_event::SyncAction>>> {
        let output = self.db.get::<ConsensusOutputSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.actions().to_owned())),
            None => Ok(None),
        }
    }

    fn get_last_checkpoint_idx(&self) -> crate::DbResult<u64> {
        match self.get_last_idx::<ConsensusStateSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_prev_checkpoint_at(&self, idx: u64) -> crate::DbResult<u64> {
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
                },
                Err(e) => return Err(DbError::Other(e.to_string())),
            }
        }

        Err(DbError::Other("Checkpoint not found".to_string()))
    }

}

#[cfg(test)]
mod tests {
    use alpen_vertex_state::consensus::ConsensusState;
    use arbitrary::{Arbitrary, Unstructured};
    use tempfile::TempDir;

    use crate::traits::ConsensusOutput;

    use super::*;

    fn setup_db() -> CsDb {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        CsDb::new(temp_dir.path()).expect("failed to create CsDb")
    }

    fn generate_arbitrary<'a, T: Arbitrary<'a> + Clone>(bytes: &'a [u8]) -> T {
        // Create an Unstructured instance and generate the arbitrary value
        let mut u = Unstructured::new(bytes);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }

    #[test]
    fn test_initialization() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = CsDb::new(temp_dir.path());
        assert!(db.is_ok());
    }

    #[test]
    fn test_last_idx() {
        let db = setup_db();
        let idx = db.get_last_idx::<ConsensusOutputSchema>().unwrap();
        assert_eq!(idx, None);
    }

    #[test]
    fn test_get_last_idx() {
        let db = setup_db();
        let idx = db.get_last_write_idx();
        assert!(idx.is_err_and(|x| matches!(x, DbError::NotBootstrapped)));
    }

    #[test]
    fn test_write_consensus_output() {
        let output: ConsensusOutput = generate_arbitrary(&[1,2,3]);
        let db = setup_db();
        let _ = db.write_consensus_output(1, output);

        let idx = db.get_last_write_idx().unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_write_consensus_checkpoint() {
        let state: ConsensusState = generate_arbitrary(&[1,2,3]);
        let db = setup_db();

        let _ = db.write_consensus_checkpoint(1, state.clone());

        let idx = db.get_last_checkpoint_idx().unwrap();
        assert_eq!(idx, 1);

        let res = db.write_consensus_checkpoint(1, state);
        assert!(res.is_err_and(|x| matches!(x, DbError::DuplicateKey(1))));
    }


    #[test]
    fn test_get_consensus_writes() {
        let output: ConsensusOutput = generate_arbitrary(&[1,2,3]);

        let db = setup_db();
        let _ = db.write_consensus_output(1, output.clone());

        let writes = db.get_consensus_writes(1).unwrap().unwrap();
        assert_eq!(&writes, output.writes());
    }

    #[test]
    fn test_get_consensus_actions() {
        let output: ConsensusOutput = generate_arbitrary(&[1,2,3]);

        let db = setup_db();
        let _ = db.write_consensus_output(1, output.clone());

        let actions = db.get_consensus_actions(1).unwrap().unwrap();
        assert_eq!(&actions, output.actions());
    }

}
