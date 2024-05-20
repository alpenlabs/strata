use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::schema::{CacheContainer, CacheDb, ChangeSet, SchemaBatch};
use crate::DbOptions;

const MAGIC_SNAPSHOT_ID: u64 = u64::MAX - 2;

/// Should be used for testing, when changes should be written to underlying DB directly.
/// Useful when caller does not want to maintain any kind of ordering or relation between changes
/// and wants them to be merged inside a database.
pub fn commit_changes_through(cache_container: &RwLock<CacheContainer>, changes: SchemaBatch) {
    // This one probably does not exist in CacheContainer
    let change_set = ChangeSet::new_with_operations(MAGIC_SNAPSHOT_ID, changes);
    let mut writer = cache_container.write().unwrap();
    writer.add_snapshot(change_set).unwrap();
    writer.commit_snapshot(&MAGIC_SNAPSHOT_ID).unwrap();
}

/// Setup simple [`CacheDb`] with ID=0, that can be used for tests.
/// Returned [`CacheContainer`] can be used together
/// with [`commit_changes_through`] to persist data to disk.
pub fn setup_cache_db_with_container(
    path: impl AsRef<std::path::Path>,
    db_options: DbOptions,
) -> (CacheDb, Arc<RwLock<CacheContainer>>) {
    let db = db_options.default_setup_db_in_path(path).unwrap();
    let to_parent = Arc::new(RwLock::new(HashMap::new()));
    let cache_container = Arc::new(RwLock::new(CacheContainer::new(
        db,
        to_parent.clone().into(),
    )));
    (
        CacheDb::new(MAGIC_SNAPSHOT_ID, cache_container.clone().into()),
        cache_container,
    )
}
