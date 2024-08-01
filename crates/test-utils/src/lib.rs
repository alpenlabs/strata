use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use alpen_express_db::database::CommonDatabase;

use alpen_express_db::stubs::{chain_state::StubChainstateDb, l2::StubL2Db};
use alpen_express_rocksdb::{ClientStateDb, L1Db, SyncEventDb};

use arbitrary::{Arbitrary, Unstructured};
use rand::RngCore;
use rockbound::rocksdb;
use tempfile::TempDir;

pub mod bitcoin;
pub mod l2;

const ARB_GEN_LEN: usize = 1 << 24; // 16 MiB

pub struct ArbitraryGenerator {
    buf: Vec<u8>,
    off: AtomicUsize,
}

impl Default for ArbitraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArbitraryGenerator {
    pub fn new() -> Self {
        Self::new_with_size(ARB_GEN_LEN)
    }

    pub fn new_with_size(n: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut buf = vec![0; n];
        rng.fill_bytes(&mut buf); // 128 wasn't enough
        let off = AtomicUsize::new(0);
        ArbitraryGenerator { buf, off }
    }

    pub fn generate<'a, T: Arbitrary<'a> + Clone>(&'a self) -> T {
        // Doing hacky atomics to make this actually be reusable, this is pretty bad.
        let off = self.off.load(Ordering::Relaxed);
        let mut u = Unstructured::new(&self.buf[off..]);
        let prev_off = u.len();
        let inst = T::arbitrary(&mut u).expect("failed to generate arbitrary instance");
        let additional_off = prev_off - u.len();
        self.off.store(off + additional_off, Ordering::Relaxed);
        inst
    }
}

pub fn get_rocksdb_tmp_instance() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let dbname = alpen_express_rocksdb::ROCKSDB_NAME;
    let cfs = alpen_express_rocksdb::STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_missing_column_families(true);
    opts.create_if_missing(true);

    let temp_dir = TempDir::new().expect("failed to create temp dir");

    let rbdb = rockbound::OptimisticTransactionDB::open(
        temp_dir.into_path(),
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
