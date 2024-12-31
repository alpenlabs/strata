use std::sync::Arc;

use strata_db::{traits::BridgeDutyIndexDatabase, DbResult};

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

    pub fn db(&self) -> &D {
        self.db.as_ref()
    }
}

inst_ops_auto! {
    (BridgeDutyIndexOps, Context<D: BridgeDutyIndexDatabase>) {
        get_index() => Option<u64>;
        set_index(index: u64) => ();
    }
}
