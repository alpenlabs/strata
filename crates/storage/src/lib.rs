mod cache;
mod exec;
mod managers;
pub mod ops;

use std::sync::Arc;

pub use managers::{checkpoint::CheckpointDbManager, l1::L1BlockManager, l2::L2BlockManager};
pub use ops::l1tx_broadcast::BroadcastDbOps;
use strata_db::traits::Database;

/// A consolidation of database managers.
#[derive(Clone)]
pub struct NodeStorage {
    l2_block_manager: Arc<L2BlockManager>,
    checkpoint_manager: Arc<CheckpointDbManager>,
}
impl NodeStorage {
    pub fn l2(&self) -> &Arc<L2BlockManager> {
        &self.l2_block_manager
    }

    pub fn checkpoint(&self) -> &Arc<CheckpointDbManager> {
        &self.checkpoint_manager
    }
}

pub fn create_node_storage<D>(db: Arc<D>, pool: threadpool::ThreadPool) -> NodeStorage
where
    D: Database + Sync + Send + 'static,
{
    let checkpoint_manager = Arc::new(CheckpointDbManager::new(pool.clone(), db.clone()));
    let l2_block_manager = Arc::new(L2BlockManager::new(pool.clone(), db.clone()));
    NodeStorage {
        checkpoint_manager,
        l2_block_manager,
    }
}
