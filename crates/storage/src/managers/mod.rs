use std::sync::Arc;

use strata_db::traits::Database;

pub mod checkpoint;
pub mod l2;

/// A consolidation of database managers and ops. Ideally ops should not be exposed.
#[derive(Clone)]
pub struct DbManager<D: Database> {
    l2_block_manager: Arc<l2::L2BlockManager>,
    checkpoint_manager: Arc<checkpoint::CheckpointDbManager>,
    db: Arc<D>, // Ultimately, we want to get rid of this
}
impl<D: Database> DbManager<D> {
    pub fn l2(&self) -> &Arc<l2::L2BlockManager> {
        &self.l2_block_manager
    }

    pub fn checkpoint(&self) -> &Arc<checkpoint::CheckpointDbManager> {
        &self.checkpoint_manager
    }

    pub fn db(&self) -> &Arc<D> {
        &self.db
    }
}

pub fn create_db_manager<D>(db: Arc<D>, pool: threadpool::ThreadPool) -> DbManager<D>
where
    D: Database + Sync + Send + 'static,
{
    let checkpoint_manager = Arc::new(checkpoint::CheckpointDbManager::new(
        pool.clone(),
        db.clone(),
    ));
    let l2_block_manager = Arc::new(l2::L2BlockManager::new(pool.clone(), db.clone()));
    DbManager {
        checkpoint_manager,
        l2_block_manager,
        db,
    }
}
