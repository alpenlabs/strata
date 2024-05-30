pub mod rocks_db_config;
pub mod schemas;
pub mod state_db;

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
