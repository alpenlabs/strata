//! Operations for reading/writing commit reveal tx related data from/to Database

use std::sync::Arc;

use strata_db::{traits::WriterDatabase, types::DataBundleIntentEntry, DbResult};
use strata_primitives::buf::Buf32;
use threadpool::ThreadPool;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: WriterDatabase + Sync + Send + 'static> {
    db: Arc<D>,
}

impl<D: WriterDatabase + Sync + Send + 'static> Context<D> {
    /// Create a `Context` for [`EnvelopeDataOps`]
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    /// Convert to [`EnvelopeDataOps`] using a [`ThreadPool`]
    pub fn into_ops(self, pool: ThreadPool) -> EnvelopeDataOps {
        EnvelopeDataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (EnvelopeDataOps, Context<D: WriterDatabase>) {
        get_entry(id: Buf32) => Option<DataBundleIntentEntry>;
        get_entry_by_idx(idx: u64) => Option<DataBundleIntentEntry>;
        get_entry_id(idx: u64) => Option<Buf32>;
        get_next_entry_idx() => u64;
        put_entry(id: Buf32, entry: DataBundleIntentEntry) => ();
    }
}

fn get_entry<D: WriterDatabase + Sync + Send + 'static>(
    ctx: &Context<D>,
    id: Buf32,
) -> DbResult<Option<DataBundleIntentEntry>> {
    ctx.db.get_entry_by_id(id)
}

fn get_entry_id<D: WriterDatabase + Sync + Send + 'static>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<Buf32>> {
    ctx.db.get_id(idx)
}

fn get_entry_by_idx<D: WriterDatabase + Sync + Send + 'static>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<DataBundleIntentEntry>> {
    let id_res = ctx.db.get_id(idx)?;
    match id_res {
        Some(id) => ctx.db.get_entry_by_id(id),
        None => Ok(None),
    }
}

fn get_next_entry_idx<D: WriterDatabase + Sync + Send + 'static>(
    ctx: &Context<D>,
) -> DbResult<u64> {
    ctx.db
        .get_last_idx()
        .map(|x| x.map(|i| i + 1).unwrap_or_default())
}

fn put_entry<D: WriterDatabase + Sync + Send + 'static>(
    ctx: &Context<D>,
    id: Buf32,
    entry: DataBundleIntentEntry,
) -> DbResult<()> {
    ctx.db.put_entry(id, entry)
}
