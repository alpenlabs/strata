//! Defines functions to operate databases.

use std::{fs, path::PathBuf, sync::Arc};

use alpen_express_rocksdb::{ROCKSDB_NAME, STORE_COLUMN_FAMILIES};
use directories::ProjectDirs;
use rockbound::{rocksdb, OptimisticTransactionDB};

/// Open or creates a rocksdb database.
///
/// # Notes
///
/// By default creates or opens a database in:
///
/// - Linux: `$HOME/.local/share/strata/rocksdb/`
/// - MacOS: `/Users/$USER/Library/Application Support/io.alpenlabs.strata/rocksdb/`
/// - Windows: `C:\Users\$USER\AppData\Roaming\alpenlabs\strata\rocksdb\data\`
///
/// Or in the specified `data_dir`.
pub(crate) fn open_rocksdb_database(
    data_dir: Option<PathBuf>,
) -> anyhow::Result<Arc<OptimisticTransactionDB>> {
    let database_dir = match data_dir {
        Some(s) => s,
        None => ProjectDirs::from("io", "alpenlabs", "strata")
            .expect("project dir should be available")
            .data_dir()
            .to_owned()
            .join("rocksdb"),
    };

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = ROCKSDB_NAME;
    let cfs = STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}
