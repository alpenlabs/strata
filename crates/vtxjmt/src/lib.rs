//! Defines the database used by the Sovereign SDK.
//!
//! - Types and traits for storing and retrieving ledger data can be found in the [`ledger_db`] module
//! - DB "Table" definitions can be found in the [`schema`] module
//! - Types and traits for storing state data can be found in the [`state_db`] module
//! - The default db configuration is generated in the [`rocks_db_config`] module
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Implements helpers for configuring RocksDB.
pub mod rocks_db_config;
/// Defines the tables used by the Sovereign SDK.
pub mod schema;
/// Implements a wrapper around [RocksDB](https://rocksdb.org/) meant for storing rollup state.
/// This is primarily used as the backing store for the [JMT(JellyfishMerkleTree)](https://docs.rs/jmt/latest/jmt/).
pub mod state_db;

/// Define namespaces at the database level
pub mod namespaces;
#[cfg(test)]
mod test_utils;

/// Options on how to setup [`rockbound::DB`] or any other persistence
pub struct DbOptions {
    /// Name of the database.
    pub(crate) name: &'static str,
    /// Sub-directory name for the [`rockbound::DB`].
    pub(crate) path_suffix: &'static str,
    /// A set of [`rockbound::schema::ColumnFamilyName`] that this db is going to use.
    pub(crate) columns: Vec<rockbound::schema::ColumnFamilyName>,
}

impl DbOptions {
    /// Setup [`rockbound::DB`] with default options
    pub fn default_setup_db_in_path(
        self,
        path: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<rockbound::DB> {
        let config = rocks_db_config::gen_rocksdb_options(&Default::default(), false);
        let db_path = path.as_ref().join(self.path_suffix);
        rockbound::DB::open(db_path, self.name, self.columns, &config)
    }
}
