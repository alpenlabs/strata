use std::fmt::Debug;
use std::sync::Arc;

use anyhow::ensure;
use jmt::storage::{HasPreimage, TreeReader};
use jmt::{KeyHash, Version};
use rockbound::cache::cache_db::CacheDb;
use rockbound::{SchemaBatch, SchemaKey};

use crate::namespaces::{KernelNamespace, Namespace, UserNamespace};
use crate::schema::namespace::{JmtNodes, JmtValues, KeyHashToKey};
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

    /// Create a new instance of [`StateDb`] from a given [`rockbound::DB`]
    pub fn with_cache_db(db: CacheDb) -> anyhow::Result<Self> {
        let next_version = Self::next_version_from(&db)?;
        Ok(Self {
            db: Arc::new(db),
            next_version,
        })
    }

    /// Returns the associated JMT handler for a given namespace
    pub fn get_jmt_handler<N: Namespace>(&self) -> JmtHandler<N> {
        JmtHandler {
            state_db: self,
            phantom: Default::default(),
        }
    }

    /// Get the next version from the database snapshot
    fn next_version_from(db_snapshot: &CacheDb) -> anyhow::Result<Version> {
        let kernel_last_key_value = db_snapshot.get_largest::<JmtNodes<KernelNamespace>>()?;
        let kernel_largest_version = kernel_last_key_value.map(|(k, _)| k.version());

        let user_last_key_value = db_snapshot.get_largest::<JmtNodes<UserNamespace>>()?;
        let user_largest_version = user_last_key_value.map(|(k, _)| k.version());

        ensure!(
            kernel_largest_version == user_largest_version,
            "Kernel and User namespaces have different largest versions: kernel={:?}, user={:?}",
            kernel_largest_version,
            user_largest_version
        );

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
            columns: UserNamespace::get_table_names()
                .into_iter()
                .chain(KernelNamespace::get_table_names())
                .collect(),
        }
    }

    /// Materializes the preimage of a hashed key into the returned [`SchemaBatch`].
    /// Note that the preimage is not checked for correctness,
    /// since the [`StateDb`] is unaware of the hash function used by the JMT.
    pub fn materialize_preimages<'a, N: Namespace>(
        items: impl IntoIterator<Item = (KeyHash, &'a SchemaKey)>,
    ) -> Result<SchemaBatch, anyhow::Error> {
        let mut batch = SchemaBatch::new();
        for (key_hash, key) in items.into_iter() {
            batch.put::<KeyHashToKey<N>>(&key_hash.0, key)?;
        }
        Ok(batch)
    }

    /// Get the current value of the `next_version` counter
    pub fn get_next_version(&self) -> Version {
        self.next_version
    }

    /// Get an optional value from the database, given a version and a key hash.
    pub fn get_value_option_by_key<N: Namespace>(
        &self,
        version: Version,
        key: &SchemaKey,
    ) -> anyhow::Result<Option<jmt::OwnedValue>> {
        let found = self.db.get_prev::<JmtValues<N>>(&(key, version))?;

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
    pub fn materialize_node_batch<N: Namespace>(
        &self,
        node_batch: &jmt::storage::NodeBatch,
        latest_preimages: Option<&SchemaBatch>,
    ) -> anyhow::Result<SchemaBatch> {
        let mut batch = SchemaBatch::new();
        for (node_key, node) in node_batch.nodes() {
            batch.put::<JmtNodes<N>>(node_key, node)?;
        }

        for ((version, key_hash), value) in node_batch.values() {
            let key_preimage = if let Some(latest_preimages) = latest_preimages {
                latest_preimages.get_value::<KeyHashToKey<N>>(&key_hash.0)?
            } else {
                None
            };
            let key_preimage = match key_preimage {
                Some(v) => v,
                None => self
                    .db
                    .read::<KeyHashToKey<N>>(&key_hash.0)?
                    .ok_or(anyhow::format_err!(
                        "Could not find preimage for key hash {key_hash:?}. Has `StateDb::put_preimage` been called for this key?"
                    ))?
            };
            batch.put::<JmtValues<N>>(&(key_preimage, *version), value)?;
        }

        Ok(batch)
    }
}

/// A simple wrapper around [`StateDb`] that implements [`TreeReader`] for a given namespace.
#[derive(Debug)]
pub struct JmtHandler<'a, N: Namespace> {
    state_db: &'a StateDb,
    phantom: std::marker::PhantomData<N>,
}

/// Default implementations of [`TreeReader`] for [`StateDb`]
impl<'a, N: Namespace> TreeReader for JmtHandler<'a, N> {
    fn get_node_option(
        &self,
        node_key: &jmt::storage::NodeKey,
    ) -> anyhow::Result<Option<jmt::storage::Node>> {
        self.state_db.db.read::<JmtNodes<N>>(node_key)
    }

    fn get_value_option(
        &self,
        version: Version,
        key_hash: KeyHash,
    ) -> anyhow::Result<Option<jmt::OwnedValue>> {
        let key_opt = self.state_db.db.read::<KeyHashToKey<N>>(&key_hash.0)?;

        if let Some(key) = key_opt {
            self.state_db.get_value_option_by_key::<N>(version, &key)
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

impl<'a, N: Namespace> HasPreimage for JmtHandler<'a, N> {
    fn preimage(&self, key_hash: KeyHash) -> anyhow::Result<Option<Vec<u8>>> {
        self.state_db.db.read::<KeyHashToKey<N>>(&key_hash.0)
    }
}

#[cfg(test)]
mod state_db_tests {
    use jmt::storage::{NodeBatch, TreeReader};
    use jmt::{JellyfishMerkleTree, KeyHash};
    use sha2::Sha256;

    use crate::namespaces::{KernelNamespace, Namespace, UserNamespace};
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
            StateDb::materialize_preimages::<UserNamespace>(vec![(key_hash, &key)]).unwrap();
        let mut batch = NodeBatch::default();
        batch.extend(vec![], vec![((0, key_hash), Some(value.to_vec()))]);
        let node_batch_schematized = state_db
            .materialize_node_batch::<UserNamespace>(&batch, Some(&preimages_schematized))
            .unwrap();

        preimages_schematized.merge(node_batch_schematized);
        commit_changes_through(&cache_container, preimages_schematized);

        // Reading back
        let state_db_handler: JmtHandler<UserNamespace> = state_db.get_jmt_handler();
        let found = state_db_handler.get_value(0, key_hash).unwrap();
        assert_eq!(found, value);

        let found = state_db
            .get_value_option_by_key::<UserNamespace>(0, &key)
            .unwrap()
            .unwrap();
        assert_eq!(found, value);
    }

    #[test]
    fn test_namespace() {
        let tempdir = tempfile::tempdir().unwrap();
        let (db_snapshot, cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(db_snapshot).unwrap();
        let user_state_db_handler: JmtHandler<'_, UserNamespace> = state_db.get_jmt_handler();
        let kernel_state_db_handler: JmtHandler<'_, KernelNamespace> = state_db.get_jmt_handler();

        let key_hash = KeyHash([1u8; 32]);
        let key = vec![2u8; 100];
        let value_1 = [8u8; 150];
        let value_2 = [100u8; 150];

        // Populate the user space of the state db with some values
        {
            let mut preimages_schematized =
                StateDb::materialize_preimages::<UserNamespace>(vec![(key_hash, &key)]).unwrap();
            let mut batch = NodeBatch::default();
            batch.extend(vec![], vec![((0, key_hash), Some(value_1.to_vec()))]);
            let node_batch_schematized = state_db
                .materialize_node_batch::<UserNamespace>(&batch, Some(&preimages_schematized))
                .unwrap();
            preimages_schematized.merge(node_batch_schematized);

            commit_changes_through(&cache_container, preimages_schematized);
        }

        // Check that user space values are read correctly from the database.
        {
            let found = user_state_db_handler.get_value(0, key_hash).unwrap();
            assert_eq!(found, value_1);
        }

        // Try to retrieve these values from the kernel space
        {
            assert!(kernel_state_db_handler.get_value(0, key_hash).is_err());
        }

        // Populate the kernel space of the state db with some values but for different version
        {
            let mut preimages_schematized =
                StateDb::materialize_preimages::<KernelNamespace>(vec![(key_hash, &key)]).unwrap();
            let mut batch = NodeBatch::default();
            batch.extend(vec![], vec![((1, key_hash), Some(value_2.to_vec()))]);
            let node_batch_schematized = state_db
                .materialize_node_batch::<KernelNamespace>(&batch, Some(&preimages_schematized))
                .unwrap();
            preimages_schematized.merge(node_batch_schematized);
            commit_changes_through(&cache_container, preimages_schematized);
        }
        // Check that the correct value is returned.
        {
            assert!(kernel_state_db_handler.get_value(0, key_hash).is_err());
            let found = kernel_state_db_handler.get_value(1, key_hash).unwrap();
            assert_eq!(found, value_2);
        }
    }

    #[test]
    fn test_root_hash_at_init() {
        let tempdir = tempfile::tempdir().unwrap();
        let (cache_db, _cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();
        let latest_version = state_db.get_next_version() - 1;
        assert_eq!(0, latest_version);

        let user_state_db_handler: JmtHandler<'_, UserNamespace> = state_db.get_jmt_handler();
        check_root_hash_at_init_handler(&user_state_db_handler);
        let kernel_state_db_handler: JmtHandler<'_, KernelNamespace> = state_db.get_jmt_handler();

        check_root_hash_at_init_handler(&kernel_state_db_handler);
    }

    #[test]
    #[ignore = "https://github.com/Sovereign-Labs/sovereign-sdk-wip/issues/648"]
    fn test_only_single_namespace_via_jmt() {
        let tempdir = tempfile::tempdir().unwrap();
        let (cache_db, cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();

        let key_hash = KeyHash([1u8; 32]);
        let key = vec![2u8; 100];
        let value = [8u8; 150];

        // Writing
        let version = state_db.get_next_version();
        let mut preimages_batch =
            StateDb::materialize_preimages::<UserNamespace>(vec![(key_hash, &key)]).unwrap();
        let db_handler: JmtHandler<'_, UserNamespace> = state_db.get_jmt_handler();
        let jmt = JellyfishMerkleTree::<JmtHandler<UserNamespace>, sha2::Sha256>::new(&db_handler);
        let (_new_root, _update_proof, tree_update) = jmt
            .put_value_set_with_proof(vec![(key_hash, Some(value.to_vec()))], version)
            .expect("JMT update must succeed");
        let node_batch = state_db
            .materialize_node_batch::<UserNamespace>(
                &tree_update.node_batch,
                Some(&preimages_batch),
            )
            .unwrap();
        preimages_batch.merge(node_batch);

        commit_changes_through(&cache_container, preimages_batch);

        drop(state_db);
        drop(cache_container);

        // Re-opening DB
        let (cache_db, _cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();

        let state_db_handler: JmtHandler<UserNamespace> = state_db.get_jmt_handler();
        let found = state_db_handler.get_value(0, key_hash).unwrap();
        assert_eq!(found, value);
    }

    #[test]
    fn test_only_single_namespace() {
        let tempdir = tempfile::tempdir().unwrap();
        let (cache_db, cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();

        let key_hash = KeyHash([1u8; 32]);
        let key = vec![2u8; 100];
        let value = [8u8; 150];

        // Writing
        let mut preimages_schematized =
            StateDb::materialize_preimages::<UserNamespace>(vec![(key_hash, &key)]).unwrap();
        let mut batch = NodeBatch::default();
        batch.extend(vec![], vec![((0, key_hash), Some(value.to_vec()))]);
        let node_batch_schematized = state_db
            .materialize_node_batch::<UserNamespace>(&batch, Some(&preimages_schematized))
            .unwrap();

        preimages_schematized.merge(node_batch_schematized);
        commit_changes_through(&cache_container, preimages_schematized);

        drop(state_db);
        drop(cache_container);

        // Re-opening DB
        let (cache_db, _cache_container) =
            setup_cache_db_with_container(tempdir.path(), StateDb::get_rockbound_options());
        let state_db = StateDb::with_cache_db(cache_db).unwrap();

        let state_db_handler: JmtHandler<UserNamespace> = state_db.get_jmt_handler();
        let found = state_db_handler.get_value(0, key_hash).unwrap();
        assert_eq!(found, value);
    }

    fn check_root_hash_at_init_handler<N: Namespace>(handler: &JmtHandler<N>) {
        let jmt = jmt::JellyfishMerkleTree::<JmtHandler<N>, Sha256>::new(handler);

        // Just pointing out the obvious.
        let root_hash = jmt.get_root_hash_option(0).unwrap();
        assert!(root_hash.is_none());
    }
}
