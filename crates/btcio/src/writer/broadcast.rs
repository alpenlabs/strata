use std::{sync::Arc, time::Duration};

use anyhow::anyhow;

use alpen_vertex_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{BlobEntry, BlobL1Status},
};

use super::{config::WriterConfig, state::WriterState};
use crate::{
    rpc::{
        traits::{L1Client, SeqL1Client},
        ClientError,
    },
    writer::{
        builder::build_inscription_txs,
        utils::{get_blob_by_idx, get_l1_tx, put_commit_reveal_txs},
    },
};

const BROADCAST_POLL_INTERVAL: u64 = 5000; // millis

/// Broadcasts the next blob to be sent
pub async fn broadcaster_task<D: SequencerDatabase + Send + Sync + 'static>(
    mut state: WriterState<D>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    loop {
        // SLEEP!
        interval.as_mut().tick().await;

        // Check from db if the last sent is confirmed because if we sent the new one before the
        // previous is confirmed, they might end up in different order

        if get_blob_by_idx(db.clone(), state.last_sent_blob_idx)
            .await?
            .map(|x| x.status == BlobL1Status::Confirmed)
            .ok_or(anyhow!("Last sent blob not found in db"))?
        {
            continue;
        }

        let next_idx = state.last_sent_blob_idx + 1;

        if let Some(blobentry) = db.sequencer_provider().get_blob_by_idx(next_idx)? {
            // Get commit reveal txns
            let commit_tx = get_l1_tx(db.clone(), blobentry.commit_txid)
                .await?
                .ok_or(anyhow!("Expected to find commit tx in db"))?;
            let reveal_tx = get_l1_tx(db.clone(), blobentry.reveal_txid)
                .await?
                .ok_or(anyhow!("Expected to find commit tx in db"))?;

            // Send
            match send_commit_reveal_txs(commit_tx, reveal_tx, rpc_client.as_ref()).await {
                Ok(_) => {
                    state.last_sent_blob_idx = next_idx;
                }
                Err(SendError::MissingOrInvalidInput) => {
                    // This is tricky, need to reconstruct commit-reveal txns. Might need to resend
                    // previous ones as well.
                    let (commit, reveal) =
                        build_inscription_txs(&blobentry.blob, &rpc_client, &config).await?;
                    let (cid, rid) = put_commit_reveal_txs(db.clone(), commit, reveal).await?;
                    let new_blobentry = BlobEntry::new_unsent(blobentry.blob.clone(), cid, rid);

                    db.sequencer_store()
                        .update_blob_by_idx(state.last_sent_blob_idx, new_blobentry)?;
                    // Do nothing, this will be sent in the next step of the loop
                }
                Err(SendError::Other(_)) => {
                    // TODO: Maybe retry?
                }
            }
        }
    }
}

enum SendError {
    MissingOrInvalidInput,
    Other(String),
}

async fn send_commit_reveal_txs(
    commit_tx_raw: Vec<u8>,
    reveal_tx_raw: Vec<u8>,
    client: &(impl SeqL1Client + L1Client),
) -> Result<(), SendError> {
    send_tx(commit_tx_raw, client).await?;
    send_tx(reveal_tx_raw, client).await?;
    Ok(())
}

async fn send_tx(tx_raw: Vec<u8>, client: &(impl SeqL1Client + L1Client)) -> Result<(), SendError> {
    match client.send_raw_transaction(tx_raw).await {
        Ok(_) => Ok(()),
        Err(ClientError::Server(-27, _)) => Ok(()), // Tx already in chain
        Err(ClientError::Server(-26, _)) => Err(SendError::MissingOrInvalidInput),
        Err(e) => Err(SendError::Other(e.to_string())),
    }
}
