//! Operations for reading/writing inscription related data from/to Database

use std::sync::Arc;

use strata_db::{
    traits::{BlobDatabase, SequencerDatabase},
    types::BlobEntry,
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
    /// Create a `Context` for [`InscriptionDataOps`]
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    /// Convert to [`InscriptionDataOps`] using a [`ThreadPool`]
    pub fn into_ops(self, pool: ThreadPool) -> InscriptionDataOps {
        InscriptionDataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (InscriptionDataOps, Context<D: SequencerDatabase>) {
        get_blob_entry(id: Buf32) => Option<BlobEntry>;
        get_blob_entry_by_idx(idx: u64) => Option<BlobEntry>;
        get_blob_entry_id(idx: u64) => Option<Buf32>;
        get_next_blob_idx() => u64;
        put_blob_entry(id: Buf32, entry: BlobEntry) => ();
    }
}

fn get_blob_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
) -> DbResult<Option<BlobEntry>> {
    let blob_db = ctx.db.blob_db();
    blob_db.get_blob_by_id(id)
}

fn get_blob_entry_id<D: SequencerDatabase>(ctx: &Context<D>, idx: u64) -> DbResult<Option<Buf32>> {
    let blob_db = ctx.db.blob_db();
    blob_db.get_blob_id(idx)
}

fn get_blob_entry_by_idx<D: SequencerDatabase>(
    ctx: &Context<D>,
    idx: u64,
) -> DbResult<Option<BlobEntry>> {
    let blob_db = ctx.db.blob_db();
    let id_res = blob_db.get_blob_id(idx)?;
    match id_res {
        Some(id) => blob_db.get_blob_by_id(id),
        None => Ok(None),
    }
}

fn get_next_blob_idx<D: SequencerDatabase>(ctx: &Context<D>) -> DbResult<u64> {
    let blob_db = ctx.db.blob_db();
    blob_db
        .get_last_blob_idx()
        .map(|x| x.map(|i| i + 1).unwrap_or_default())
}

fn put_blob_entry<D: SequencerDatabase>(
    ctx: &Context<D>,
    id: Buf32,
    entry: BlobEntry,
) -> DbResult<()> {
    let blob_db = ctx.db.blob_db();
    blob_db.put_blob_entry(id, entry)
}
