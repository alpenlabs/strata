//! Implements a wrapper around [RocksDB](https://rocksdb.org/) meant for storing rollup state.
//! This is primarily used as the backing store for the [JMT(JellyfishMerkleTree)](https://docs.rs/jmt/latest/jmt/).
//! Adapted from sov-sdk

use std::fmt::Debug;
use std::sync::Arc;

use jmt::storage::{HasPreimage, TreeReader};
use jmt::{KeyHash, Version};
use rockbound::cache::cache_db::CacheDb;
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
    next_version: Version,
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
            next_version,
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

        let user_last_key_value = db_snapshot.get_largest::<JmtNodes>()?;
        let user_largest_version = user_last_key_value.map(|(k, _)| k.version());

        let next_version = user_largest_version
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

    /// Materializes the preimage of a hashed key into the returned [`SchemaBatch`].
    /// Note that the preimage is not checked for correctness,
    /// since the [`StateDb`] is unaware of the hash function used by the JMT.
    pub fn materialize_preimages<'a>(
        items: impl IntoIterator<Item = (KeyHash, &'a SchemaKey)>,
    ) -> Result<SchemaBatch, anyhow::Error> {
        let mut batch = SchemaBatch::new();
        for (key_hash, key) in items.into_iter() {
            batch.put::<KeyHashToKey>(&key_hash.0, key)?;
        }
        Ok(batch)
    }

    /// Get the current value of the `next_version` counter
    pub fn get_next_version(&self) -> Version {
        self.next_version
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

    /// Converts [`jmt::storage::NodeBatch`] into serialized [`SchemaBatch`].
    /// Optional `latest_preimages` is for preimages from the current slot,
    /// which might not be available in the [`StateDb`] yet.
    pub fn materialize_node_batch(
        &self,
        node_batch: &jmt::storage::NodeBatch,
        latest_preimages: Option<&SchemaBatch>,
    ) -> anyhow::Result<SchemaBatch> {
        let mut batch = SchemaBatch::new();
        for (node_key, node) in node_batch.nodes() {
            batch.put::<JmtNodes>(node_key, node)?;
        }

        for ((version, key_hash), value) in node_batch.values() {
            let key_preimage = if let Some(latest_preimages) = latest_preimages {
                latest_preimages.get_value::<KeyHashToKey>(&key_hash.0)?
            } else {
                None
            };
            let key_preimage = match key_preimage {
                Some(v) => v,
                None => self
                    .db
                    .read::<KeyHashToKey>(&key_hash.0)?
                    .ok_or(anyhow::format_err!(
                        "Could not find preimage for key hash {key_hash:?}. Has `StateDb::put_preimage` been called for this key?"
                    ))?
            };
            batch.put::<JmtValues>(&(key_preimage, *version), value)?;
        }

        Ok(batch)
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

impl<'a> HasPreimage for JmtHandler<'a> {
    fn preimage(&self, key_hash: KeyHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.state_db.db.read::<KeyHashToKey>(&key_hash.0)
    }
}

#[cfg(test)]
mod state_db_tests {
    use jmt::storage::{NodeBatch, TreeReader};
    use jmt::KeyHash;
    use sha2::Sha256;

    use crate::state_db::{JmtHandler, StateDb};
    use crate::test_utils::{commit_changes_through, setup_cache_db_with_container};

    #[test]
    fn test_simple() {
        let tempdir = tempfile::tempdir().unwrap();
        let (db_snapshot, cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = &StateDb::with_cache_db(db_snapshot).unwrap();

        let key_hash = KeyHash([1u8; 32]);
        let key = vec![2u8; 100];
        let value = [8u8; 150];

        // Writing
        let mut preimages_schematized =
            StateDb::materialize_preimages(vec![(key_hash, &key)]).unwrap();
        let mut batch = NodeBatch::default();
        batch.extend(vec![], vec![((0, key_hash), Some(value.to_vec()))]);
        let node_batch_schematized = state_db
            .materialize_node_batch(&batch, Some(&preimages_schematized))
            .unwrap();

        preimages_schematized.merge(node_batch_schematized);
        commit_changes_through(&cache_container, preimages_schematized);

        // Reading back
        let state_db_handler: JmtHandler = state_db.get_jmt_handler();
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
        let (cache_db, _cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();
        let latest_version = state_db.get_next_version() - 1;
        assert_eq!(0, latest_version);

        let user_state_db_handler: JmtHandler<'_> = state_db.get_jmt_handler();
        check_root_hash_at_init_handler(&user_state_db_handler);
        let kernel_state_db_handler: JmtHandler<'_> = state_db.get_jmt_handler();

        check_root_hash_at_init_handler(&kernel_state_db_handler);
    }

    fn check_root_hash_at_init_handler(handler: &JmtHandler) {
        let jmt = jmt::JellyfishMerkleTree::<JmtHandler, Sha256>::new(handler);

        // Just pointing out the obvious.
        let root_hash = jmt.get_root_hash_option(0).unwrap();
        assert!(root_hash.is_none());
    }
}
