use std::sync::Arc;

use bitcoin::hashes::Hash;
use bitcoin::{consensus::serialize, Transaction};

use alpen_vertex_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobEntry,
};
use alpen_vertex_primitives::buf::Buf32;

// Helper function to fetch a blob entry from withing tokio
pub async fn get_blob_by_idx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    idx: u64,
) -> anyhow::Result<Option<BlobEntry>> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_provider().get_blob_by_idx(idx)?)).await?
}

// Helper function to update a blob entry by index from withing tokio
pub async fn update_blob_by_idx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    idx: u64,
    blob_entry: BlobEntry,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        Ok(db.sequencer_store().update_blob_by_idx(idx, blob_entry)?)
    })
    .await?
}

// Helper function to fetch a l1tx from withing tokio
pub async fn get_l1_tx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    txid: Buf32,
) -> anyhow::Result<Option<Vec<u8>>> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_provider().get_l1_tx(txid)?)).await?
}

// Helper function to store commit reveal txs
pub async fn put_commit_reveal_txs<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    commit_tx: Transaction,
    reveal_tx: Transaction,
) -> anyhow::Result<(Buf32, Buf32)> {
    let cid: Buf32 = commit_tx
        .compute_txid()
        .as_raw_hash()
        .to_byte_array()
        .into();
    let rid: Buf32 = reveal_tx
        .compute_txid()
        .as_raw_hash()
        .to_byte_array()
        .into();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        Ok(db.sequencer_store().put_commit_reveal_txs(
            cid,
            serialize(&commit_tx),
            rid,
            serialize(&reveal_tx),
        )?)
    })
    .await??;
    Ok((cid, rid))
}
