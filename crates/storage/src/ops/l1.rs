//! L1 block data operation interface.

use std::sync::Arc;

use alpen_express_db::traits::*;
use alpen_express_primitives::{l1::*, prelude::*};
use alpen_express_state::l1::L1BlockId;

use crate::exec::*;

/// Database context for the operation interface.
pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> L1DataOps {
        L1DataOps::new(pool, Arc::new(self))
    }
}

inst_ops! {
    (L1DataOps, Context<D: Database>) {
        put_block_data(idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) => ();
        revert_to_height(height: u64) => ();
        get_chain_tip() => Option<u64>;
        get_block_manifest(height: u64) => Option<L1BlockManifest>;
        get_blkid_range(start: u64, end: u64) => Vec<Buf32>;
        get_block_txs(blk_idx: u64) => Option<Vec<L1TxRef>>;
        get_tx(tx_ref: L1TxRef) => Option<L1Tx>;
    }
}

fn put_block_data<D: Database>(
    context: &Context<D>,
    idx: u64,
    mf: L1BlockManifest,
    txs: Vec<L1Tx>,
) -> DbResult<()> {
    let l1_store = context.db.l1_store();
    l1_store.put_block_data(idx, mf, txs)
}

fn revert_to_height<D: Database>(context: &Context<D>, height: u64) -> DbResult<()> {
    let l1_store = context.db.l1_store();
    l1_store.revert_to_height(height)
}

fn get_chain_tip<D: Database>(context: &Context<D>) -> DbResult<Option<u64>> {
    let l1_prov = context.db.l1_provider();
    l1_prov.get_chain_tip()
}

fn get_block_manifest<D: Database>(
    context: &Context<D>,
    height: u64,
) -> DbResult<Option<L1BlockManifest>> {
    let l1_prov = context.db.l1_provider();
    l1_prov.get_block_manifest(height)
}

fn get_blkid_range<D: Database>(
    context: &Context<D>,
    start: u64,
    end: u64,
) -> DbResult<Vec<Buf32>> {
    let l1_prov = context.db.l1_provider();
    l1_prov.get_blockid_range(start, end)
}

fn get_block_txs<D: Database>(
    context: &Context<D>,
    blk_idx: u64,
) -> DbResult<Option<Vec<L1TxRef>>> {
    let l1_prov = context.db.l1_provider();
    l1_prov.get_block_txs(blk_idx)
}

fn get_tx<D: Database>(context: &Context<D>, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>> {
    let l1_prov = context.db.l1_provider();
    l1_prov.get_tx(tx_ref)
}

// TODO mmr-related functions
