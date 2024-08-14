//! L2 block data manager.  Maintains references to the handles and stuff.

use std::sync::Arc;

//use tokio::sync::oneshot;
//use tracing::*;

use alpen_express_db::traits::*;
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};

use crate::exec::*;

pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}

inst_ops! {
    (L2DataOps, Context<D: Database>) {
        get_block(L2BlockId) => Option<L2BlockBundle> [get_block_blocking, get_block_async];
        get_blocks_at_height(u64) => Vec<L2BlockId> [get_blocks_at_height_blocking, get_blocks_at_height_async];
        get_block_status(L2BlockId) => Option<BlockStatus> [get_block_status_blocking, get_block_status_async];
        put_block(L2BlockBundle) => () [put_block_blocking, put_block_async];
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
