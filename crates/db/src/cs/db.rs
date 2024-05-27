
use anyhow::anyhow;
use rockbound::{schema::KeyEncoder, Schema, SchemaBatch, DB};
use rocksdb::{Options, ReadOptions};

use std::path::Path;

use crate::{errors::DbError, traits::{ConsensusStateProvider, ConsensusStateStore}};

use super::schemas::ConsensusStateSchema;

const DB_NAME: &str = "cs_db";

pub struct CsDb {
    db: DB
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
            crate::cs::schemas::ConsensusStateSchema::COLUMN_FAMILY_NAME
        ];
        let store = Self {
            db: rockbound::DB::open(path, DB_NAME, column_families, &db_opts)?
        };
        Ok(store)
    }
}

impl ConsensusStateStore for CsDb {
    fn write_consensus_output(&self, idx: u64, output: crate::traits::ConsensusOutput) -> crate::DbResult<()> {
        match self.get_last_write_idx()? {
            Some(last_idx) => {
                if last_idx + 1 != idx {
                    return Err(DbError::OooInsert("Consensus store", idx));
                }
            },
            None => {
                if idx != 1{
                    return Err(DbError::OooInsert("Consensus store", idx));
                }
            }
        }
        let mut batch = SchemaBatch::new();
        batch.put::<ConsensusStateSchema>(&idx, &output)?;
        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn write_consensus_checkpoint(&self, idx: u64, state: alpen_vertex_state::consensus::ConsensusState) -> crate::DbResult<()> {
        todo!()
    }

}

impl ConsensusStateProvider for CsDb {
    fn get_last_write_idx(&self) -> crate::DbResult<Option<u64>> {
        let mut iterator = self.db.iter::<ConsensusStateSchema>()?;
        iterator.seek_to_last();
        match iterator.rev().next() {
            Some(res) => {
                let (tip, _) = res?.into_tuple();
                Ok(Some(tip))
            },
            None => Ok(None)
        }
    }

    fn get_consensus_writes(&self, idx: u64) -> crate::DbResult<Option<Vec<alpen_vertex_state::consensus::ConsensusWrite>>> {
        let output = self.db.get::<ConsensusStateSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.writes)),
            None => Ok(None)
        }
    }

    fn get_consensus_actions(&self, idx: u64) -> crate::DbResult<Option<Vec<alpen_vertex_state::sync_event::SyncAction>>> {
        let output = self.db.get::<ConsensusStateSchema>(&idx)?;
        match output {
            Some(out) => Ok(Some(out.actions)),
            None => Ok(None)
        }
    }

    fn get_last_checkpoint_idx(&self) -> crate::DbResult<u64> {
        todo!()
    }

    fn get_prev_checkpoint_at(&self, idx: u64) -> crate::DbResult<u64> {
        todo!()
    }

}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use tempfile::TempDir;

    use crate::traits::ConsensusOutput;

    use super::*;

    fn setup_db() -> CsDb {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        CsDb::new(temp_dir.path()).expect("failed to create CsDb")
    }

    fn generate_arbitrary<'a, T: Arbitrary<'a> + Clone>() -> T {
        let mut u = Unstructured::new(&[1, 2, 3]);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }

    #[test]
    fn test_initialization() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = CsDb::new(temp_dir.path());
        assert!(db.is_ok());
    }

    #[test]
    fn test_get_last_idx() {
        let db = setup_db();
        let idx = db.get_last_write_idx().unwrap();
        assert_eq!(idx, None);
    }

    #[test]
    fn test_write_consensus_output() {
        let output: ConsensusOutput = generate_arbitrary();
        let db = setup_db();
        let _ = db.write_consensus_output(1, output);

        let idx = db.get_last_write_idx().unwrap().unwrap();
        assert_eq!(idx, 1);

    }



}