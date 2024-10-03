use std::sync::Arc;

use alpen_express_db::{traits::BridgeDutyIndexDatabase, DbResult};

use crate::exec::*;

/// Database context for a database operation interface.
pub struct Context<D: BridgeDutyIndexDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: BridgeDutyIndexDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BridgeDutyIndexOps {
        BridgeDutyIndexOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (BridgeDutyIndexOps, Context<D: BridgeDutyIndexDatabase>) {
        get_index() => Option<u64>;
        set_index(index: u64) => ();
    }
}

fn get_index<D: BridgeDutyIndexDatabase + Sync + Send + 'static>(
    context: &Context<D>,
) -> DbResult<Option<u64>> {
    context.db.get_index()
}

fn set_index<D: BridgeDutyIndexDatabase + Sync + Send + 'static>(
    context: &Context<D>,
    index: u64,
) -> DbResult<()> {
    context.db.set_index(index)
}
