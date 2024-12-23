use std::sync::Arc;

use strata_db::traits::Database;

pub mod checkpoint;
pub mod l2;

/// A consolidation of database managers and ops. Ideally ops should not be exposed.
pub struct DbManagers {
    l2_block_manager: Arc<l2::L2BlockManager>,
    checkpoint_manager: Arc<checkpoint::CheckpointDbManager>,
}
impl DbManagers {
    pub fn l2(&self) -> Arc<l2::L2BlockManager> {
        self.l2_block_manager.clone()
    }

    pub fn checkpoint(&self) -> Arc<checkpoint::CheckpointDbManager> {
        self.checkpoint_manager.clone()
    }
}

pub fn create_db_managers<D>(db: Arc<D>, pool: threadpool::ThreadPool) -> DbManagers
where
    D: Database + Sync + Send + 'static,
{
    let checkpoint_manager = Arc::new(checkpoint::CheckpointDbManager::new(
        pool.clone(),
        db.clone(),
    ));
    let l2_block_manager = Arc::new(l2::L2BlockManager::new(pool.clone(), db.clone()));
    DbManagers {
        checkpoint_manager,
        l2_block_manager,
    }
}
