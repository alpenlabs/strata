use std::{fs, path::PathBuf, sync::Arc};

use rockbound::rocksdb;

pub fn open_rocksdb_database() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let mut database_dir = PathBuf::default();
    database_dir.push("rocksdb_prover");

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = strata_rocksdb::ROCKSDB_NAME;
    let cfs = strata_rocksdb::PROVER_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = rockbound::OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}
