//! Operations for reading/writing commit reveal tx related data from/to Database

use std::sync::Arc;

use strata_db::{
    traits::{SequencerDatabase, WriterDatabase},
    types::CommitRevealEntry,
    DbResult,
};
use strata_primitives::buf::Buf32;
use threadpool::ThreadPool;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: SequencerDatabase> {
    db: Arc<D>,
}

impl<D: SequencerDatabase + Sync + Send + 'static> Context<D> {
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
    (EnvelopeDataOps, Context<D: SequencerDatabase>) {
        get_entry(id: Buf32) => Option<CommitRevealEntry>;
        get_entry_by_idx(idx: u64) => Option<CommitRevealEntry>;
        get_entry_id(idx: u64) => Option<Buf32>;
        get_next_entry_idx() => u64;
        put_entry(id: Buf32, entry: CommitRevealEntry) => ();
    }
}

fn get_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
) -> DbResult<Option<CommitRevealEntry>> {
    let blob_db = ctx.db.commit_reveal_db();
    blob_db.get_entry_by_id(id)
}

fn get_entry_id<D: SequencerDatabase>(ctx: &Context<D>, idx: u64) -> DbResult<Option<Buf32>> {
    let blob_db = ctx.db.commit_reveal_db();
    blob_db.get_id(idx)
}

fn get_entry_by_idx<D: SequencerDatabase>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<CommitRevealEntry>> {
    let blob_db = ctx.db.commit_reveal_db();
    let id_res = blob_db.get_id(idx)?;
    match id_res {
        Some(id) => blob_db.get_entry_by_id(id),
        None => Ok(None),
    }
}

fn get_next_entry_idx<D: SequencerDatabase>(ctx: &Context<D>) -> DbResult<u64> {
    let blob_db = ctx.db.commit_reveal_db();
    blob_db
        .get_last_idx()
        .map(|x| x.map(|i| i + 1).unwrap_or_default())
}

fn put_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
    entry: CommitRevealEntry,
) -> DbResult<()> {
    let blob_db = ctx.db.commit_reveal_db();
    blob_db.put_entry(id, entry)
}
