use std::sync::Arc;

use arbitrary::{Arbitrary, Unstructured};
use rand::Rng;
use tempfile::TempDir;

pub mod bitcoin;

pub struct ArbitraryGenerator {
    buffer: Vec<u8>,
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

pub fn get_rocksdb_tmp_instance() -> anyhow::Result<Arc<rockbound::DB>> {
    let dbname = alpen_vertex_db::ROCKSDB_NAME;
    let cfs = alpen_vertex_db::STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_missing_column_families(true);
    opts.create_if_missing(true);

    let temp_dir = TempDir::new().expect("failed to create temp dir");

    let rbdb = rockbound::DB::open(
        &temp_dir.into_path(),
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}
