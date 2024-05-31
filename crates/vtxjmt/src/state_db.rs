//! Implements a wrapper around [RocksDB](https://rocksdb.org/) meant for storing rollup state.
//! This is primarily used as the backing store for the [JMT(JellyfishMerkleTree)](https://docs.rs/jmt/latest/jmt/).
//! Adapted from sov-sdk

use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use jmt::storage::{HasPreimage, TreeReader, TreeWriter};
use jmt::{KeyHash, Version};
use rockbound::cache::cache_db::CacheDb;
use rockbound::cache::change_set::ChangeSet;
use rockbound::schema::ColumnFamilyName;
use rockbound::{Schema, SchemaBatch, SchemaKey};

use crate::schemas::{JmtNodes, JmtValues, KeyHashToKey};
use crate::DbOptions;

/// A typed wrapper around the db for storing rollup state. Internally,
/// this is roughly just an [`Arc<rockbound::CacheDB>`].
#[derive(Debug, Clone)]
pub struct StateDb {
    /// The underlying [`CacheDb`] that plays as local cache and pointer to previous snapshots and/or [`rockbound::DB`]
    db: Arc<CacheDb>,
    /// The [`Version`] that will be used for the next batch of writes to the DB
    /// This [`Version`] is also used for querying data,
    /// so if this instance of StateDb is used as read-only, it won't see newer data.
    next_version: Arc<Mutex<Version>>,
}

impl StateDb {
    const DB_PATH_SUFFIX: &'static str = "state";
    const DB_NAME: &'static str = "state-db";

    /// Returns the table names used in JMT
    fn get_column_names() -> [ColumnFamilyName; 3] {
        [
            KeyHashToKey::COLUMN_FAMILY_NAME,
            JmtNodes::COLUMN_FAMILY_NAME,
            JmtValues::COLUMN_FAMILY_NAME,
        ]
    }

    /// Create a new instance of [`StateDb`] from a given [`rockbound::DB`]
    pub fn with_cache_db(db: CacheDb) -> anyhow::Result<Self> {
        let next_version = Self::next_version_from(&db)?;
        Ok(Self {
            db: Arc::new(db),
            next_version: Arc::new(Mutex::new(next_version)),
        })
    }

    /// Returns the associated JMT handler 
    pub fn get_jmt_handler(&self) -> JmtHandler {
        JmtHandler {
            state_db: self,
        }
    }

    /// Get the next version from the database snapshot
    fn next_version_from(db_snapshot: &CacheDb) -> anyhow::Result<Version> {

        let last_key_value = db_snapshot.get_largest::<JmtNodes>()?;
        let largest_version = last_key_value.map(|(k, _)| k.version());

        let next_version = largest_version
            .unwrap_or_default()
            .checked_add(1)
            .expect("JMT Version overflow. Is is over");

        Ok(next_version)
    }

    /// [`DbOptions`] for [`StateDb`].
    pub fn get_rockbound_options() -> DbOptions {
        DbOptions {
            name: Self::DB_NAME,
            path_suffix: Self::DB_PATH_SUFFIX,
            columns: Self::get_column_names().to_vec() 
        }
    }

    /// Convert it to [`ChangeSet`] which cannot be edited anymore
    pub fn freeze(self) -> anyhow::Result<ChangeSet> {
        let inner = Arc::into_inner(self.db).ok_or(anyhow::anyhow!(
            "StateDb underlying CacheDb has more than 1 strong references"
        ))?;
        Ok(ChangeSet::from(inner))
    }

    /// Put the preimage of a hashed key into the database. Note that the preimage is not checked for correctness,
    /// since the DB is unaware of the hash function used by the JMT.
    pub fn put_preimages<'a>(
        &self,
        items: impl IntoIterator<Item = (KeyHash, &'a SchemaKey)>,
    ) -> Result<(), anyhow::Error> {
        let mut batch = SchemaBatch::new();
        for (key_hash, key) in items.into_iter() {
            batch.put::<KeyHashToKey>(&key_hash.0, key)?;
        }
        self.db.write_many(batch)?;
        Ok(())
    }

    /// Increment the `next_version` counter by 1.
    pub fn inc_next_version(&self) {
        let mut version = self.next_version.lock().unwrap();
        *version += 1;
    }

    /// Get the current value of the `next_version` counter
    pub fn get_next_version(&self) -> Version {
        let version = self.next_version.lock().unwrap();
        *version
    }


    /// Get an optional value from the database, given a version and a key hash.
    pub fn get_value_option_by_key(
        &self,
        version: Version,
        key: &SchemaKey,
    ) -> anyhow::Result<Option<jmt::OwnedValue>> {
        let found = self.db.get_prev::<JmtValues>(&(key, version))?;

        match found {
            Some(((found_key, found_version), value)) => {
                if &found_key == key {
                    anyhow::ensure!(found_version <= version, "Bug! iterator isn't returning expected values. expected a version <= {version:} but found {found_version:}");
                    Ok(value)
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }
}

/// A simple wrapper around [`StateDb`] that implements [`TreeReader`] .
#[derive(Debug)]
pub struct JmtHandler<'a> {
    state_db: &'a StateDb,
}

/// Default implementations of [`TreeReader`] for [`StateDb`]
impl<'a> TreeReader for JmtHandler<'a> {
    fn get_node_option(
        &self,
        node_key: &jmt::storage::NodeKey,
    ) -> anyhow::Result<Option<jmt::storage::Node>> {
        self.state_db.db.read::<JmtNodes>(node_key)
    }

    fn get_value_option(
        &self,
        version: Version,
        key_hash: KeyHash,
    ) -> anyhow::Result<Option<jmt::OwnedValue>> {
        let key_opt = self.state_db.db.read::<KeyHashToKey>(&key_hash.0)?;

        if let Some(key) = key_opt {
            self.state_db.get_value_option_by_key(version, &key)
        } else {
            Ok(None)
        }
    }

    fn get_rightmost_leaf(
        &self,
    ) -> anyhow::Result<Option<(jmt::storage::NodeKey, jmt::storage::LeafNode)>> {
        todo!("StateDb does not support [`TreeReader::get_rightmost_leaf`] yet")
    }
}

/// Default implementation of [`TreeWriter`] for [`StateDb`]
impl<'a> TreeWriter for JmtHandler<'a> {
    fn write_node_batch(&self, node_batch: &jmt::storage::NodeBatch) -> anyhow::Result<()> {
        let mut batch = SchemaBatch::new();
        for (node_key, node) in node_batch.nodes() {
            batch.put::<JmtNodes>(node_key, node)?;
        }

        for ((version, key_hash), value) in node_batch.values() {
            let key_preimage =
                self
                    .state_db
                    .db
                    .read::<KeyHashToKey>(&key_hash.0)?
                    .ok_or(anyhow::format_err!(
                                    "Could not find preimage for key hash {key_hash:?}. Has `StateDb::put_preimage` been called for this key?"
                                ))?;
            batch.put::<JmtValues>(&(key_preimage, *version), value)?;
        }
        self.state_db.db.write_many(batch)?;
        Ok(())
    }
}

impl<'a> HasPreimage for JmtHandler<'a> {
    fn preimage(&self, key_hash: KeyHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.state_db.db.read::<KeyHashToKey>(&key_hash.0)
    }
}

#[cfg(test)]
mod state_db_tests {
    use std::path;
    use std::sync::{Arc, RwLock};

    use jmt::storage::{NodeBatch, TreeReader, TreeWriter};
    use jmt::KeyHash;
    use sha2::Sha256;

    use rockbound::cache::cache_container::CacheContainer;
    use rockbound::cache::cache_db::CacheDb;

    use crate::state_db::{JmtHandler, StateDb};

    fn init_cache_db(path: &path::Path) -> CacheDb {
        let db = StateDb::get_rockbound_options()
            .default_setup_db_in_path(path)
            .unwrap();
        let cache_container =
            CacheContainer::new(db, Arc::new(RwLock::new(Default::default())).into());

        CacheDb::new(0, Arc::new(RwLock::new(cache_container)).into())
    }

    #[test]
    fn test_simple() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_snapshot = init_cache_db(tempdir.path());
        let state_db = &StateDb::with_cache_db(db_snapshot).unwrap();
        let state_db_handler = state_db.get_jmt_handler();
        let key_hash = KeyHash([1u8; 32]);
        let key = vec![2u8; 100];
        let value = [8u8; 150];

        state_db
            .put_preimages(vec![(key_hash, &key)])
            .unwrap();
        let mut batch = NodeBatch::default();
        batch.extend(vec![], vec![((0, key_hash), Some(value.to_vec()))]);
        state_db_handler.write_node_batch(&batch).unwrap();

        let found = state_db_handler.get_value(0, key_hash).unwrap();
        assert_eq!(found, value);

        let found = state_db
            .get_value_option_by_key(0, &key)
            .unwrap()
            .unwrap();
        assert_eq!(found, value);
    }

    #[test]
    fn test_root_hash_at_init() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_snapshot = init_cache_db(tempdir.path());
        let db = StateDb::with_cache_db(db_snapshot).unwrap();
        let latest_version = db.get_next_version() - 1;
        assert_eq!(0, latest_version);

        let state_db_handler: JmtHandler<'_> = db.get_jmt_handler();

        let jmt = jmt::JellyfishMerkleTree::<JmtHandler, Sha256>::new(&state_db_handler);

        // Just pointing out the obvious.
        let root_hash = jmt.get_root_hash_option(0).unwrap();
        assert!(root_hash.is_none());
    }

}
