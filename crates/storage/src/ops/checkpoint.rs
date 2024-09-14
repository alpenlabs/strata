//! Checkpoint Proof data operation interface.

use std::sync::Arc;

use alpen_express_db::{traits::*, types::CheckpointEntry};

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> CheckpointDataOps {
        CheckpointDataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (CheckpointDataOps, Context<D: Database>) {
        get_batch_checkpoint(idx: u64) => Option<CheckpointEntry>;
        put_batch_checkpoint(idx: u64, entry: CheckpointEntry) => ();
    }
}

fn get_batch_checkpoint<D: Database>(
    context: &Context<D>,
    idx: u64,
) -> DbResult<Option<CheckpointEntry>> {
    let checkpt_prov = context.db.checkpoint_provider();
    checkpt_prov.get_batch_checkpoint(idx)
}

fn put_batch_checkpoint<D: Database>(
    context: &Context<D>,
    idx: u64,
    entry: CheckpointEntry,
) -> DbResult<()> {
    let checkpt_store = context.db.checkpoint_store();
    checkpt_store.put_batch_checkpoint(idx, entry)
}
