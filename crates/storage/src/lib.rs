mod cache;
mod exec;
mod managers;
pub mod ops;

use std::sync::Arc;

pub use managers::{
    chainstate::ChainstateManager, checkpoint::CheckpointDbManager,
    client_state::ClientStateManager, l1::L1BlockManager, l2::L2BlockManager,
};
pub use ops::l1tx_broadcast::BroadcastDbOps;
use strata_db::{traits::Database, DbResult};

/// A consolidation of database managers.
// TODO move this to its own module
#[derive(Clone)]
pub struct NodeStorage {
    l1_block_manager: Arc<L1BlockManager>,
    l2_block_manager: Arc<L2BlockManager>,
    chainstate_manager: Arc<ChainstateManager>,
    client_state_manager: Arc<ClientStateManager>,

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

    pub fn client_state(&self) -> &Arc<ClientStateManager> {
        &self.client_state_manager
    }

    pub fn checkpoint(&self) -> &Arc<CheckpointDbManager> {
        &self.checkpoint_manager
    }
}

/// Given a raw database, creates storage managers and returns a [`NodeStorage`]
/// instance around the underlying raw database.
pub fn create_node_storage<D>(db: Arc<D>, pool: threadpool::ThreadPool) -> DbResult<NodeStorage>
where
    D: Database + Sync + Send + 'static,
{
    let l1_block_manager = Arc::new(L1BlockManager::new(pool.clone(), db.clone()));
    let l2_block_manager = Arc::new(L2BlockManager::new(pool.clone(), db.clone()));
    let client_state_manager = Arc::new(ClientStateManager::new(pool.clone(), db.clone())?);
    let chainstate_manager = Arc::new(ChainstateManager::new(pool.clone(), db.clone()));

    // (see above)
    let checkpoint_manager = Arc::new(CheckpointDbManager::new(pool.clone(), db.clone()));

    Ok(NodeStorage {
        l1_block_manager,
        l2_block_manager,
        client_state_manager,
        chainstate_manager,

        // (see above)
        checkpoint_manager,
    })
}
