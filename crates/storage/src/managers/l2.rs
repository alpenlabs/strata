//! L2 block data manager.  Maintains references to the handles and stuff.

use std::sync::Arc;

//use tokio::sync::oneshot;
//use tracing::*;

use alpen_express_db::traits::*;
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};

use crate::exec::*;

inst_ops! {
    (L2DataOps, Context<D: Database>) {
        imp_get_block => get_block_blocking, get_block_async; L2BlockId => Option<L2BlockBundle>
    }
}

pub struct Context<D: Database> {
    db: Arc<D>,
}

fn imp_get_block<D: Database>(
    context: &Context<D>,
    t: L2BlockId,
) -> DbResult<Option<L2BlockBundle>> {
    let l2_prov = context.db.l2_provider();
    l2_prov.get_block_data(t)
}
