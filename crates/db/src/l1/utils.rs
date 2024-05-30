use std::path::Path;

use rockbound::{schema::ColumnFamilyName, DB};
use rocksdb::Options;

use crate::STORE_COLUMN_FAMILIES;

const DB_NAME: &str = "l1_db";

pub fn get_db_for_l1_store(path: &Path) -> anyhow::Result<DB> {
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
}
