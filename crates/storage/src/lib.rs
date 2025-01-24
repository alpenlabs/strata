mod cache;
mod exec;
mod managers;
pub mod ops;

use std::sync::Arc;

pub use managers::{
    chainstate::ChainstateManager, checkpoint::CheckpointDbManager, l1::L1BlockManager,
    l2::L2BlockManager,
};
pub use ops::l1tx_broadcast::BroadcastDbOps;
use strata_db::traits::Database;

/// A consolidation of database managers.
#[derive(Clone)]
pub struct NodeStorage {
    l1_block_manager: Arc<L1BlockManager>,
    l2_block_manager: Arc<L2BlockManager>,
    chainstate_manager: Arc<ChainstateManager>,

    // TODO maybe move this into a different one?
    checkpoint_manager: Arc<CheckpointDbManager>,
}

impl NodeStorage {
    pub fn l1(&self) -> &Arc<L1BlockManager> {
        &self.l1_block_manager
    }

    pub fn l2(&self) -> &Arc<L2BlockManager> {
        &self.l2_block_manager
    }

    pub fn chainstate(&self) -> &Arc<ChainstateManager> {
        &self.chainstate_manager
    }

    pub fn checkpoint(&self) -> &Arc<CheckpointDbManager> {
        &self.checkpoint_manager
    }
}

pub fn create_node_storage<D>(db: Arc<D>, pool: threadpool::ThreadPool) -> NodeStorage
where
    D: Database + Sync + Send + 'static,
{
    let l1_block_manager = Arc::new(L1BlockManager::new(pool.clone(), db.clone()));
    let l2_block_manager = Arc::new(L2BlockManager::new(pool.clone(), db.clone()));
    let chainstate_manager = Arc::new(ChainstateManager::new(pool.clone(), db.clone()));
    let checkpoint_manager = Arc::new(CheckpointDbManager::new(pool.clone(), db.clone()));

    NodeStorage {
        l1_block_manager,
        l2_block_manager,
        chainstate_manager,
        checkpoint_manager,
    }
}
