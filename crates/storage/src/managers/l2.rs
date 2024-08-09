//! L2 block data manager.  Maintains references to the handles and stuff.

use std::sync::Arc;

use alpen_express_db::{traits::*, DbResult};
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};
use threadpool::ThreadPool;

use crate::exec::{inst_ops, OpShim};

pub struct L2DataManager {
    pool: ThreadPool,
    imp_get_block: OpShim<L2BlockId, Option<L2BlockBundle>>,
}

impl L2DataManager {
    pub fn _new_manual<D: Database + Sync + Send + 'static>(
        pool: ThreadPool,
        ctx: Arc<Context<D>>,
    ) -> Self {
        Self {
            pool,
            imp_get_block: {
                let ctx = ctx.clone();
                OpShim::wrap(move |arg| imp_get_block(ctx.as_ref(), arg))
            },
        }
    }
}

inst_ops! {
    (L2DataManager => pool, Context<D: Database>) {
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
