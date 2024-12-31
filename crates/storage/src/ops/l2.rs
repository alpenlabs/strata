//! L2 block data operation interface.

use std::sync::Arc;

use strata_db::traits::*;
use strata_state::{block::L2BlockBundle, id::L2BlockId};

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> L2DataOps {
        L2DataOps::new(pool, Arc::new(self))
    }

    pub fn db(&self) -> &impl L2BlockDatabase {
        self.db.l2_db().as_ref()
    }
}

inst_ops_auto! {
    (L2DataOps, Context<D: Database>) {
        get_block_data(id: L2BlockId) => Option<L2BlockBundle>;
        get_blocks_at_height(h: u64) => Vec<L2BlockId>;
        get_block_status(id: L2BlockId) => Option<BlockStatus>;
        put_block_data(block: L2BlockBundle) => ();
        set_block_status(id: L2BlockId, status: BlockStatus) => ();
    }
}
