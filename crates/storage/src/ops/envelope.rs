//! Operations for reading/writing envelope related data from/to Database

use std::sync::Arc;

use strata_db::{
    traits::{L1PayloadDatabase, SequencerDatabase},
    types::PayloadEntry,
    DbResult,
};
use strata_primitives::buf::Buf32;
use threadpool::ThreadPool;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D> {
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
        get_payload_entry(id: Buf32) => Option<PayloadEntry>;
        get_payload_entry_by_idx(idx: u64) => Option<PayloadEntry>;
        get_payload_entry_id(idx: u64) => Option<Buf32>;
        get_next_payload_idx() => u64;
        put_payload_entry(id: Buf32, entry: PayloadEntry) => ();
    }
}

fn get_payload_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
) -> DbResult<Option<PayloadEntry>> {
    let payload_db = ctx.db.payload_db();
    payload_db.get_payload_by_id(id)
}

fn get_payload_entry_id<D: SequencerDatabase>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<Buf32>> {
    let payload_db = ctx.db.payload_db();
    payload_db.get_payload_id(idx)
}

fn get_payload_entry_by_idx<D: SequencerDatabase>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<PayloadEntry>> {
    let payload_db = ctx.db.payload_db();
    let id_res = payload_db.get_payload_id(idx)?;
    match id_res {
        Some(id) => payload_db.get_payload_by_id(id),
        None => Ok(None),
    }
}

/// Returns zero if there are no elements else last index incremented by 1.
fn get_next_payload_idx<D: SequencerDatabase>(ctx: &Context<D>) -> DbResult<u64> {
    let payload_db = ctx.db.payload_db();
    Ok(payload_db
        .get_last_payload_idx()?
        .map(|i| i + 1)
        .unwrap_or_default())
}

fn put_payload_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
    entry: PayloadEntry,
) -> DbResult<()> {
    let payload_db = ctx.db.payload_db();
    payload_db.put_payload_entry(id, entry)
}
