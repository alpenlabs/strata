// TODO rename/remove this module to avoid confusion with new tx broadcaster module

use std::{sync::Arc, time::Duration};

use alpen_express_rpc_types::L1Status;
use anyhow::anyhow;
use bitcoin::{consensus::deserialize, Txid};
use bitcoincore_rpc_async::Error as RpcError;
use tokio::sync::RwLock;
use tracing::*;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobL1Status,
};

use crate::{
    rpc::traits::BitcoinClient,
    writer::utils::{get_blob_by_idx, get_l1_tx},
};

// TODO: make this configurable, possibly get from Params
const BROADCAST_POLL_INTERVAL: u64 = 1000; // millis

/// Broadcasts the next blob to be sent
pub async fn broadcaster_task<D: SequencerDatabase + Send + Sync + 'static>(
    next_publish_blob_idx: u64,
    rpc_client: Arc<impl BitcoinClient>,
    db: Arc<D>,
    l1_status: Arc<RwLock<L1Status>>,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's broadcaster task");
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    let mut curr_idx = next_publish_blob_idx;

    loop {
        // SLEEP!
        interval.as_mut().tick().await;

        // Check from db if the previous published blob is confirmed/finalized. Because if not, they
        // might end up in different order
        if curr_idx > 0
            && !get_blob_by_idx(db.clone(), curr_idx - 1)
                .await?
                .map(|x| x.status == BlobL1Status::Confirmed || x.status == BlobL1Status::Finalized)
                .ok_or(anyhow!("Last published blob not found in db"))?
        {
            continue;
        }

        if let Some(mut blobentry) = db.sequencer_provider().get_blob_by_idx(curr_idx)? {
            match blobentry.status {
                BlobL1Status::Unsigned | BlobL1Status::NeedsResign => {
                    continue;
                }
                BlobL1Status::Confirmed | BlobL1Status::Published | BlobL1Status::Finalized => {
                    curr_idx += 1;
                    continue;
                }
                BlobL1Status::Unpublished => {
                    // do the publishing work below
                }
            }
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
                    debug!("Successfully sent: {}", blobentry.reveal_txid.to_string());
                    blobentry.status = BlobL1Status::Published;
                    db.sequencer_store()
                        .update_blob_by_idx(curr_idx, blobentry.clone())?;
                    // Update L1 status
                    {
                        let mut l1st = l1_status.write().await;
                        let txid: Txid = deserialize(blobentry.reveal_txid.0.as_slice())?;
                        l1st.last_published_txid = Some(txid.to_string());
                        l1st.published_inscription_count += 1;
                        // TODO: add last update
                        #[cfg(debug_assertions)]
                        debug!("Updated l1 status: {:?}", l1st);
                    }
                    curr_idx += 1;
                }
                Err(e) => {
                    // DEBUG: What the hell is this?
                    // FIXME: remove me
                    #[cfg(debug_assertions)]
                    dbg!(&e);
                    // Means need to Resign/Republish
                    blobentry.status = BlobL1Status::NeedsResign;

                    db.sequencer_store()
                        .update_blob_by_idx(curr_idx, blobentry)?;
                }
            }
        } else {
            debug!(%curr_idx, "No blob found");
        }
    }
}

async fn send_commit_reveal_txs(
    commit_tx_raw: Vec<u8>,
    reveal_tx_raw: Vec<u8>,
    client: &impl BitcoinClient,
) -> Result<(), RpcError> {
    send_tx(commit_tx_raw, client).await?;
    send_tx(reveal_tx_raw, client).await?;
    Ok(())
}

async fn send_tx(tx_raw: Vec<u8>, client: &impl BitcoinClient) -> Result<Txid, RpcError> {
    client.send_raw_transaction(tx_raw).await
}
