//! Operations for reading/writing inscription related data to db

use std::sync::Arc;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobEntry,
    DbResult,
};
use alpen_express_primitives::buf::Buf32;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: SequencerDatabase> {
    db: Arc<D>,
}

impl<D: SequencerDatabase + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> InscriptionDataOps {
        InscriptionDataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (InscriptionDataOps, Context<D: SequencerDatabase>) {
        get_blob_entry(id: Buf32) => Option<BlobEntry>;
        get_blob_entry_by_idx(idx: u64) => Option<BlobEntry>;
        get_blob_id(idx: u64) => Option<Buf32>;
        get_next_blob_idx() => u64;
        put_blob_entry(id: Buf32, entry: BlobEntry) => Option<u64>;
    }
}

fn get_blob_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
) -> DbResult<Option<BlobEntry>> {
    let provider = ctx.db.sequencer_provider();
    provider.get_blob_by_id(id)
}

fn get_blob_id<D: SequencerDatabase>(ctx: &Context<D>, idx: u64) -> DbResult<Option<Buf32>> {
    let provider = ctx.db.sequencer_provider();
    provider.get_blob_id(idx)
}

fn get_blob_entry_by_idx<D: SequencerDatabase>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<BlobEntry>> {
    let provider = ctx.db.sequencer_provider();
    match provider.get_blob_id(idx)? {
        Some(id) => provider.get_blob_by_id(id),
        None => Ok(None),
    }
}

fn get_next_blob_idx<D: SequencerDatabase>(ctx: &Context<D>) -> DbResult<u64> {
    let provider = ctx.db.sequencer_provider();
    provider
        .get_last_blob_idx()
        .map(|x| x.map(|i| i + 1).unwrap_or_default())
}

/// Inserts blob entry to database, returns None if already exists, else returns Some(u64)
fn put_blob_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
    entry: BlobEntry,
) -> DbResult<Option<u64>> {
    let provider = ctx.db.sequencer_provider();
    if provider.get_blob_by_id(id)?.is_some() {
        return Ok(None);
    }
    let store = ctx.db.sequencer_store();
    Ok(Some(store.add_new_blob_entry(id, entry)?))
}
