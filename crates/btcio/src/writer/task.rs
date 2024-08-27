use std::{sync::Arc, time::Duration};

use alpen_express_db::{
    traits::SequencerDatabase,
    types::{BlobEntry, BlobL1Status, ExcludeReason, L1TxEntry, L1TxStatus},
};
use alpen_express_rpc_types::L1Status;
use alpen_express_state::da_blob::BlobIntent;
use express_storage::ops::inscription::{Context, InscriptionDataOps};
use express_tasks::TaskExecutor;
use tokio::sync::RwLock;
use tracing::*;

use super::config::WriterConfig;
use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::traits::{L1Client, SeqL1Client},
    writer::signer::create_and_sign_blob_inscriptions,
};

// TODO: from config
const FINALITY_DEPTH: u64 = 6;

/// A handle to the Inscription task. This is basically just a db wrapper for now
pub struct InscriptionHandle {
    ops: Arc<InscriptionDataOps>,
}

impl InscriptionHandle {
    pub fn new(ops: Arc<InscriptionDataOps>) -> Self {
        Self { ops }
    }

    pub fn submit_intent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());
        debug!("Received intent: {}", intent.commitment());
        if self
            .ops
            .get_blob_entry_blocking(*intent.commitment())?
            .is_some()
        {
            warn!("Received duplicate intent {:?}", intent.commitment());
            return Ok(());
        }

        Ok(self
            .ops
            .put_blob_entry_blocking(*intent.commitment(), entry)?)
    }

    pub async fn submit_intent_async(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());
        debug!("Received intent: {}", intent.commitment());

        if self
            .ops
            .get_blob_entry_async(*intent.commitment())
            .await?
            .is_some()
        {
            warn!("Received duplicate intent {:?}", intent.commitment());
            return Ok(());
        }
        Ok(self
            .ops
            .put_blob_entry_async(*intent.commitment(), entry)
            .await?)
    }
}

pub fn start_inscription_tasks<D: SequencerDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
    l1_status: Arc<RwLock<L1Status>>,
    pool: threadpool::ThreadPool,
    bcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<InscriptionHandle> {
    let ops = Arc::new(Context::new(db).into_ops(pool));
    let insc_mgr = InscriptionHandle::new(ops.clone());

    let next_watch_blob_idx = get_next_blobidx_to_watch(ops.as_ref())?;

    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(
            next_watch_blob_idx,
            rpc_client,
            config,
            ops,
            bcast_handle,
            l1_status,
        )
        .await
        .unwrap()
    });

    Ok(insc_mgr)
}

/// Looks into the database from descending index order till it reaches 0 or Finalized blobentry
/// from which the rest of the entries should be watched
fn get_next_blobidx_to_watch(insc_ops: &InscriptionDataOps) -> anyhow::Result<u64> {
    let mut next_idx = insc_ops.get_next_blob_idx_blocking()?;

    while next_idx > 0 {
        let Some(blob) = insc_ops.get_blob_entry_by_idx_blocking(next_idx - 1)? else {
            break;
        };
        if blob.status == BlobL1Status::Finalized {
            break;
        };
        next_idx -= 1;
    }
    Ok(next_idx)
}

/// Watches for inscription transactions status in bitcoin. Note that this watches for each
/// inscription until it is confirmed
pub async fn watcher_task(
    next_blbidx_to_watch: u64,
    rpc_client: Arc<impl L1Client + SeqL1Client>,
    config: WriterConfig,
    insc_ops: Arc<InscriptionDataOps>,
    bcast_handle: Arc<L1BroadcastHandle>,
    l1_status: Arc<RwLock<L1Status>>,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's watcher task");
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    let mut curr_blobidx = next_blbidx_to_watch;
    loop {
        interval.as_mut().tick().await;

        if let Some(blobentry) = insc_ops.get_blob_entry_by_idx_async(curr_blobidx).await? {
            match blobentry.status {
                // If unsigned or needs resign, create new signed commit/reveal txs and update the
                // entry
                BlobL1Status::Unsigned | BlobL1Status::NeedsResign => {
                    debug!(%curr_blobidx, "Processing unsigned blobentry");
                    let (cid, rid) = create_and_sign_blob_inscriptions(
                        &blobentry,
                        &bcast_handle,
                        rpc_client.clone(),
                        &config,
                    )
                    .await?;
                    let mut updated_entry = blobentry.clone();
                    updated_entry.status = BlobL1Status::Unpublished;
                    updated_entry.commit_txid = cid;
                    updated_entry.reveal_txid = rid;
                    update_existing_entry(curr_blobidx, updated_entry, &insc_ops).await?;

                    debug!(%curr_blobidx, "Signed blob");
                }
                // If finalized, nothing to do, move on to process next entry
                BlobL1Status::Finalized => {
                    curr_blobidx += 1;
                }
                // If excluded, nothing to do, move on to process next entry
                // TODO: create new batch and send?
                BlobL1Status::Excluded => {
                    warn!(%curr_blobidx, "blobentry is excluded, might need to recreate duty");
                    curr_blobidx += 1;
                }
                // If entry is signed but not finalized or excluded yet, check broadcast txs status
                BlobL1Status::Published | BlobL1Status::Confirmed | BlobL1Status::Unpublished => {
                    debug!(%curr_blobidx, "Checking blobentry's broadcast status");
                    let commit_tx = bcast_handle
                        .get_tx_entry_by_id_async(blobentry.commit_txid)
                        .await?
                        .expect("Commit tx associated with blobentry not found in broadcast db");
                    let reveal_tx = bcast_handle
                        .get_tx_entry_by_id_async(blobentry.reveal_txid)
                        .await?
                        .expect("Reveal tx associated with blobentry not found in broadcast db");

                    let new_status =
                        determine_blob_next_status(&commit_tx, &reveal_tx, &blobentry.status)?;

                    // Update L1 status. Since we are processing one blobentry at a time, if the
                    // entry is finalized/confirmed, then it means it is published as well
                    if new_status == BlobL1Status::Published
                        || new_status == BlobL1Status::Confirmed
                        || new_status == BlobL1Status::Finalized
                    {
                        let mut status = l1_status.write().await;
                        status.last_published_txid = Some(blobentry.reveal_txid.into());
                        status.published_inscription_count += 1;
                    }

                    // Update blobentry with new status
                    let mut updated_entry = blobentry.clone();
                    updated_entry.status = new_status.clone();
                    update_existing_entry(curr_blobidx, updated_entry, &insc_ops).await?;

                    if new_status == BlobL1Status::Finalized
                        || new_status == BlobL1Status::Confirmed
                    {
                        curr_blobidx += 1;
                    }
                }
            }
        } else {
            // No blob exists, just continue the loop to wait for blob's presence in db
            info!(%curr_blobidx, "Waiting for blobentry to be present in db");
        }
    }
}

async fn update_existing_entry(
    idx: u64,
    updated_entry: BlobEntry,
    insc_ops: &InscriptionDataOps,
) -> anyhow::Result<()> {
    let msg = format!("Expect to find blobentry {idx} in db");
    let id = insc_ops.get_blob_entry_id_async(idx).await?.expect(&msg);
    Ok(insc_ops.put_blob_entry_async(id, updated_entry).await?)
}

/// Based on the corresponding broadcast commit and reveal transactions' status, determine the
/// status of the blob entry
fn determine_blob_next_status(
    ctx: &L1TxEntry,
    rtx: &L1TxEntry,
    curr_status: &BlobL1Status,
) -> anyhow::Result<BlobL1Status> {
    let status = match (&ctx.status, &rtx.status) {
        // If reveal is finalized, both are finalized
        (_, L1TxStatus::Finalized { .. }) => BlobL1Status::Finalized,
        // If reveal is confirmed, both are confirmed
        (_, L1TxStatus::Confirmed { .. }) => BlobL1Status::Confirmed,
        // If reveal is published, both are published
        (_, L1TxStatus::Published) => BlobL1Status::Published,
        // If commit is excluded, both are excluded
        (
            L1TxStatus::Excluded {
                reason: ExcludeReason::MissingInputsOrSpent,
            },
            _,
        ) => BlobL1Status::NeedsResign,
        (L1TxStatus::Excluded { reason }, _) => {
            // TODO: error or have a separate status?
            warn!(?reason, "Inscriptions could not be included in the chain");
            curr_status.clone()
        }
        (_, _) => curr_status.clone(),
    };
    Ok(status)
}

#[cfg(test)]
mod test {
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::writer::test_utils::get_insc_ops;

    #[test]
    fn test_initialize_writer_state_no_last_blob_idx() {
        let iops = get_insc_ops();

        let nextidx = iops.get_next_blob_idx_blocking().unwrap();
        assert_eq!(nextidx, 0);

        let idx = get_next_blobidx_to_watch(&iops).unwrap();

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_blobs() {
        let iops = get_insc_ops();

        let mut e1: BlobEntry = ArbitraryGenerator::new().generate();
        e1.status = BlobL1Status::Finalized;
        let blob_hash: Buf32 = [1; 32].into();
        iops.put_blob_entry_blocking(blob_hash, e1).unwrap();
        let expected_idx = iops.get_next_blob_idx_blocking().unwrap();

        let mut e2: BlobEntry = ArbitraryGenerator::new().generate();
        e2.status = BlobL1Status::Published;
        let blob_hash: Buf32 = [2; 32].into();
        iops.put_blob_entry_blocking(blob_hash, e2).unwrap();

        let mut e3: BlobEntry = ArbitraryGenerator::new().generate();
        e3.status = BlobL1Status::Unsigned;
        let blob_hash: Buf32 = [3; 32].into();
        iops.put_blob_entry_blocking(blob_hash, e3).unwrap();

        let mut e4: BlobEntry = ArbitraryGenerator::new().generate();
        e4.status = BlobL1Status::Unsigned;
        let blob_hash: Buf32 = [4; 32].into();
        iops.put_blob_entry_blocking(blob_hash, e4).unwrap();

        let idx = get_next_blobidx_to_watch(&iops).unwrap();

        assert_eq!(idx, expected_idx);
    }
}
