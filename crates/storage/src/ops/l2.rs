//! L2 block data operation interface.

use std::sync::Arc;

use alpen_express_db::traits::*;
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};

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
}

inst_ops! {
    (L2DataOps, Context<D: Database>) {
        get_block(id: L2BlockId) => Option<L2BlockBundle>;
        get_blocks_at_height(h: u64) => Vec<L2BlockId>;
        get_block_status(id: L2BlockId) => Option<BlockStatus>;
        put_block(block: L2BlockBundle) => ();
        put_block_status(id: L2BlockId, status: BlockStatus) => ();
    }
}

fn get_block<D: Database>(context: &Context<D>, id: L2BlockId) -> DbResult<Option<L2BlockBundle>> {
    let l2_prov = context.db.l2_provider();
    l2_prov.get_block_data(id)
}

fn get_blocks_at_height<D: Database>(context: &Context<D>, h: u64) -> DbResult<Vec<L2BlockId>> {
    let l2_prov = context.db.l2_provider();
    l2_prov.get_blocks_at_height(h)
}

fn get_block_status<D: Database>(
    context: &Context<D>,
    id: L2BlockId,
) -> DbResult<Option<BlockStatus>> {
    let l2_prov = context.db.l2_provider();
    l2_prov.get_block_status(id)
}

fn put_block<D: Database>(context: &Context<D>, block: L2BlockBundle) -> DbResult<()> {
    let l2_store = context.db.l2_store();
    l2_store.put_block_data(block)
}

fn put_block_status<D: Database>(
    context: &Context<D>,
    id: L2BlockId,
    status: BlockStatus,
) -> DbResult<()> {
    let l2_store = context.db.l2_store();
    l2_store.set_block_status(id, status)
}
