
use anyhow::anyhow;
use rockbound::{schema::KeyEncoder, Schema, SchemaBatch, DB};
use rocksdb::{Options, ReadOptions};

use std::path::Path;

use crate::traits::{ConsensusStateProvider, ConsensusStateStore};

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
}

impl ConsensusStateProvider for CsDb {
    fn get_consensus_actions(&self, idx: u64) -> crate::DbResult<Option<Vec<alpen_vertex_state::sync_event::SyncAction>>> {
        todo!()
    }

    fn get_consensus_state(&self, idx: u64) -> crate::DbResult<Option<crate::traits::ConsensusOutput>> {
        todo!()
    }

    fn get_last_idx(&self) -> crate::DbResult<u64> {
        todo!()
    }
}