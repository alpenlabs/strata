use std::sync::Arc;

use alpen_vertex_db::{
    database::CommonDatabase,
    stubs::{chain_state::StubChainstateDb, l2::StubL2Db},
    ClientStateDb, L1Db, SyncEventDb,
};
use arbitrary::{Arbitrary, Unstructured};
use rand::Rng;
use tempfile::TempDir;

pub mod bitcoin;

pub struct ArbitraryGenerator {
    buffer: Vec<u8>,
}

impl Default for ArbitraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArbitraryGenerator {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        // NOTE: 128 should be enough for testing purposes. Change to 256 as needed
        let buffer: Vec<u8> = (0..128).map(|_| rng.gen()).collect();
        ArbitraryGenerator { buffer }
    }

    pub fn generate<'a, T: Arbitrary<'a> + Clone>(&'a self) -> T {
        let mut u = Unstructured::new(&self.buffer);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }
}

pub fn get_rocksdb_tmp_instance() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let dbname = alpen_vertex_db::ROCKSDB_NAME;
    let cfs = alpen_vertex_db::STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_missing_column_families(true);
    opts.create_if_missing(true);

    let temp_dir = TempDir::new().expect("failed to create temp dir");

    let rbdb = rockbound::OptimisticTransactionDB::open(
        &temp_dir.into_path(),
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}

pub fn get_common_db(
) -> Arc<CommonDatabase<L1Db, StubL2Db, SyncEventDb, ClientStateDb, StubChainstateDb>> {
    let rbdb = get_rocksdb_tmp_instance().unwrap();
    let l1_db = Arc::new(L1Db::new(rbdb.clone()));
    let l2_db = Arc::new(StubL2Db::new());
    let sync_ev_db = Arc::new(SyncEventDb::new(rbdb.clone()));
    let cs_db = Arc::new(ClientStateDb::new(rbdb.clone()));
    let chst_db = Arc::new(StubChainstateDb::new());

    Arc::new(CommonDatabase::new(
        l1_db, l2_db, sync_ev_db, cs_db, chst_db,
    ))
}
