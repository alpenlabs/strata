
use anyhow::anyhow;
use rockbound::{schema::KeyEncoder, Schema, SchemaBatch, DB};
use rocksdb::{Options, ReadOptions};

use std::path::Path;

use crate::traits::{ConsensusStateProvider, ConsensusStateStore};

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
        todo!()
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
        todo!()
    }


    fn get_consensus_actions(&self, idx: u64) -> crate::DbResult<Option<Vec<alpen_vertex_state::sync_event::SyncAction>>> {
        todo!()
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
    use tempfile::TempDir;

    use super::*;

    fn setup_db() -> CsDb {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        CsDb::new(temp_dir.path()).expect("failed to create CsDb")
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



}