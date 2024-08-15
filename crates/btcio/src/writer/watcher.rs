use std::{sync::Arc, time::Duration};

use tracing::*;

use alpen_express_db::{
    traits::SequencerDatabase,
    types::{BlobEntry, BlobL1Status},
};
use bitcoin::{hashes::Hash, Txid};

use crate::{
    rpc::traits::{L1Client, SeqL1Client},
    writer::utils::{create_and_sign_blob_inscriptions, get_blob_by_idx},
};

use super::{config::WriterConfig, utils::update_blob_by_idx};

const FINALITY_DEPTH: u64 = 6;

/// Watches for inscription transactions status in bitcoin
pub async fn watcher_task<D: SequencerDatabase + Send + Sync + 'static>(
    next_to_watch: u64,
    rpc_client: Arc<impl L1Client + SeqL1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's watcher task");
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    let mut curr_blobidx = next_to_watch;
    loop {
        interval.as_mut().tick().await;

        if let Some(blobentry) = get_blob_by_idx(db.clone(), curr_blobidx).await? {
            match blobentry.status {
                BlobL1Status::Published | BlobL1Status::Confirmed => {
                    debug!(%curr_blobidx, "blobentry is published or confirmed");
                    let confs = check_confirmations_and_update_entry(
                        rpc_client.clone(),
                        curr_blobidx,
                        blobentry,
                        db.clone(),
                    )
                    .await?;
                    if confs > 0 {
                        curr_blobidx += 1;
                    }
                }
                BlobL1Status::Unsigned | BlobL1Status::NeedsResign => {
                    debug!(%curr_blobidx, "blobentry is unsigned or needs resign");
                    create_and_sign_blob_inscriptions(
                        curr_blobidx,
                        db.clone(),
                        rpc_client.clone(),
                        &config,
                    )
                    .await?;
                }
                BlobL1Status::Finalized => {
                    debug!(%curr_blobidx, "blobentry is finalized");
                    curr_blobidx += 1;
                }
                BlobL1Status::Unpublished => {
                    debug!(%curr_blobidx, "blobentry is unpublished;");
                } // Do Nothing
            }
        } else {
            // No blob exists, just continue the loop and thus wait for blob to be present in db
        }
    }
}

async fn check_confirmations_and_update_entry<D: SequencerDatabase + Send + Sync + 'static>(
    rpc_client: Arc<impl L1Client>,
    curr_blobidx: u64,
    mut blobentry: BlobEntry,
    db: Arc<D>,
) -> anyhow::Result<u64> {
    let txid = Txid::from_slice(blobentry.reveal_txid.0.as_slice())?;
    let confs = rpc_client.get_transaction_confirmations(txid).await?;
    // If confs is 0 then it is yet in mempool
    // TODO: But if confs is error(saying txn not found, TODO: check this) then it
    // could possibly have reorged and we might need to
    // resign/resend it

    if confs >= 1 {
        // blob is confirmed, mark it as confirmed
        if confs >= FINALITY_DEPTH {
            blobentry.status = BlobL1Status::Finalized;
        } else {
            blobentry.status = BlobL1Status::Confirmed;
        }

        // Update this in db
        update_blob_by_idx(db.clone(), curr_blobidx, blobentry.clone()).await?;

        // Also set blobs that are deep enough as finalized
        if curr_blobidx < FINALITY_DEPTH {
            // No need to check for finalized entries
            return Ok(confs);
        }
        let finidx = curr_blobidx - FINALITY_DEPTH;
        let startidx = if confs >= finidx { 0 } else { finidx - confs };
        for idx in startidx..=finidx {
            if let Some(blobentry) = get_blob_by_idx(db.clone(), idx).await? {
                if blobentry.status == BlobL1Status::Finalized {
                    continue;
                }
                let mut blobentry = blobentry.clone();
                blobentry.status = BlobL1Status::Finalized;

                update_blob_by_idx(db.clone(), idx, blobentry.clone()).await?;
            }
        }
    }
    Ok(confs)
}
