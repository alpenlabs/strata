use std::{sync::Arc, time::Duration};

use strata_db::{
    traits::SequencerDatabase,
    types::{BlobEntry, BlobL1Status, L1TxStatus},
};
use strata_state::da_blob::{BlobDest, BlobIntent};
use strata_status::StatusChannel;
use strata_storage::ops::inscription::{Context, InscriptionDataOps};
use strata_tasks::TaskExecutor;
use tracing::*;

use super::config::WriterConfig;
use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::traits::{Reader, Signer, Wallet},
    status::{apply_status_updates, L1StatusUpdate},
    writer::{builder::InscriptionError, signer::create_and_sign_blob_inscriptions},
};

/// A handle to the Inscription task.
pub struct InscriptionHandle {
    ops: Arc<InscriptionDataOps>,
}

impl InscriptionHandle {
    pub fn new(ops: Arc<InscriptionDataOps>) -> Self {
        Self { ops }
    }

    pub fn submit_intent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        if intent.dest() != BlobDest::L1 {
            warn!(commitment = %intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());
        debug!(commitment = %intent.commitment(), "Received intent");
        if self
            .ops
            .get_blob_entry_blocking(*intent.commitment())?
            .is_some()
        {
            warn!(commitment = %intent.commitment(), "Received duplicate intent");
            return Ok(());
        }

        Ok(self
            .ops
            .put_blob_entry_blocking(*intent.commitment(), entry)?)
    }

    pub async fn submit_intent_async(&self, intent: BlobIntent) -> anyhow::Result<()> {
        if intent.dest() != BlobDest::L1 {
            warn!(commitment = %intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());
        debug!(commitment = %intent.commitment(), "Received intent");

        if self
            .ops
            .get_blob_entry_async(*intent.commitment())
            .await?
            .is_some()
        {
            warn!(commitment = %intent.commitment(), "Received duplicate intent");
            return Ok(());
        }
        Ok(self
            .ops
            .put_blob_entry_async(*intent.commitment(), entry)
            .await?)
    }
}

/// Starts the inscription task.
///
/// This creates an [`InscriptionHandle`] and spawns a watcher task that watches the status of
/// incriptions in bitcoin.
///
/// # Returns
///
/// [`Result<InscriptionHandle>`](anyhow::Result)
pub fn start_inscription_task<D: SequencerDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    bitcoin_client: Arc<impl Reader + Wallet + Signer + Send + Sync + 'static>,
    config: WriterConfig,
    db: Arc<D>,
    status_channel: StatusChannel,
    pool: threadpool::ThreadPool,
    broadcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<Arc<InscriptionHandle>> {
    let inscription_data_ops = Arc::new(Context::new(db).into_ops(pool));
    let next_watch_blob_idx = get_next_blobidx_to_watch(inscription_data_ops.as_ref())?;

    let inscription_handle = Arc::new(InscriptionHandle::new(inscription_data_ops.clone()));

    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(
            next_watch_blob_idx,
            bitcoin_client,
            config,
            inscription_data_ops,
            broadcast_handle,
            status_channel,
        )
        .await
    });

    Ok(inscription_handle)
}

/// Looks into the database from descending index order till it reaches 0 or `Finalized`
/// [`BlobEntry`] from which the rest of the [`BlobEntry`]s should be watched.
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
/// Watches for inscription transactions status in the Bitcoin blockchain.
///
/// # Note
///
/// The inscription will be monitored until it acquires the status of
/// [`BlobL1Status::Finalized`]
pub async fn watcher_task(
    next_blbidx_to_watch: u64,
    bitcoin_client: Arc<impl Reader + Wallet + Signer>,
    config: WriterConfig,
    insc_ops: Arc<InscriptionDataOps>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    status_channel: StatusChannel,
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
                    debug!(?blobentry.status, %curr_blobidx, "Processing unsigned blobentry");
                    match create_and_sign_blob_inscriptions(
                        &blobentry,
                        &broadcast_handle,
                        bitcoin_client.clone(),
                        &config,
                    )
                    .await
                    {
                        Ok((cid, rid)) => {
                            let mut updated_entry = blobentry.clone();
                            updated_entry.status = BlobL1Status::Unpublished;
                            updated_entry.commit_txid = cid;
                            updated_entry.reveal_txid = rid;
                            update_existing_entry(curr_blobidx, updated_entry, &insc_ops).await?;

                            debug!(%curr_blobidx, "Signed blob");
                        }
                        Err(InscriptionError::NotEnoughUtxos(required, available)) => {
                            // Just wait till we have enough utxos and let the status be `Unsigned`
                            // or `NeedsResign`
                            // Maybe send an alert
                            error!(%required, %available, "Not enough utxos available to create commit/reveal transaction");
                        }
                        e => {
                            e?;
                        }
                    }
                }
                // If finalized, nothing to do, move on to process next entry
                BlobL1Status::Finalized => {
                    curr_blobidx += 1;
                }
                // If entry is signed but not finalized or excluded yet, check broadcast txs status
                BlobL1Status::Published | BlobL1Status::Confirmed | BlobL1Status::Unpublished => {
                    debug!(%curr_blobidx, "Checking blobentry's broadcast status");
                    let commit_tx = broadcast_handle
                        .get_tx_entry_by_id_async(blobentry.commit_txid)
                        .await?;
                    let reveal_tx = broadcast_handle
                        .get_tx_entry_by_id_async(blobentry.reveal_txid)
                        .await?;

                    match (commit_tx, reveal_tx) {
                        (Some(ctx), Some(rtx)) => {
                            let new_status = determine_blob_next_status(&ctx.status, &rtx.status);
                            debug!(?new_status, "The next status for blob");

                            update_l1_status(&blobentry, &new_status, &status_channel).await;

                            // Update blobentry with new status
                            let mut updated_entry = blobentry.clone();
                            updated_entry.status = new_status.clone();
                            update_existing_entry(curr_blobidx, updated_entry, &insc_ops).await?;

                            if new_status == BlobL1Status::Finalized {
                                curr_blobidx += 1;
                            }
                        }
                        _ => {
                            warn!(%curr_blobidx, "Corresponding commit/reveal entry for blobentry not found in broadcast db. Sign and create transactions again.");
                            let mut updated_entry = blobentry.clone();
                            updated_entry.status = BlobL1Status::Unsigned;
                            update_existing_entry(curr_blobidx, updated_entry, &insc_ops).await?;
                        }
                    }
                }
            }
        } else {
            // No blob exists, just continue the loop to wait for blob's presence in db
            info!(%curr_blobidx, "Waiting for blobentry to be present in db");
        }
    }
}

async fn update_l1_status(
    blobentry: &BlobEntry,
    new_status: &BlobL1Status,
    status_channel: &StatusChannel,
) {
    // Update L1 status. Since we are processing one blobentry at a time, if the entry is
    // finalized/confirmed, then it means it is published as well
    if *new_status == BlobL1Status::Published
        || *new_status == BlobL1Status::Confirmed
        || *new_status == BlobL1Status::Finalized
    {
        let status_updates = [
            L1StatusUpdate::LastPublishedTxid(blobentry.reveal_txid.into()),
            L1StatusUpdate::IncrementInscriptionCount,
        ];
        apply_status_updates(&status_updates, status_channel).await;
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

/// Determine the status of the `BlobEntry` based on the status of its commit and reveal
/// transactions in bitcoin.
fn determine_blob_next_status(
    commit_status: &L1TxStatus,
    reveal_status: &L1TxStatus,
) -> BlobL1Status {
    match (&commit_status, &reveal_status) {
        // If reveal is finalized, both are finalized
        (_, L1TxStatus::Finalized { .. }) => BlobL1Status::Finalized,
        // If reveal is confirmed, both are confirmed
        (_, L1TxStatus::Confirmed { .. }) => BlobL1Status::Confirmed,
        // If reveal is published regardless of commit, the blob is published
        (_, L1TxStatus::Published) => BlobL1Status::Published,
        // if commit has invalid inputs, needs resign
        (L1TxStatus::InvalidInputs, _) => BlobL1Status::NeedsResign,
        // If commit is unpublished, both are upublished
        (L1TxStatus::Unpublished, _) => BlobL1Status::Unpublished,
        // If commit is published but not reveal, the blob is unpublished
        (_, L1TxStatus::Unpublished) => BlobL1Status::Unpublished,
        // If reveal has invalid inputs, these need resign because we can do nothing with just
        // commit tx confirmed. This should not occur in practice
        (_, L1TxStatus::InvalidInputs) => BlobL1Status::NeedsResign,
    }
}

#[cfg(test)]
mod test {
    use strata_primitives::buf::Buf32;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::writer::test_utils::get_inscription_ops;

    #[test]
    fn test_initialize_writer_state_no_last_blob_idx() {
        let iops = get_inscription_ops();

        let nextidx = iops.get_next_blob_idx_blocking().unwrap();
        assert_eq!(nextidx, 0);

        let idx = get_next_blobidx_to_watch(&iops).unwrap();

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_blobs() {
        let iops = get_inscription_ops();

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

    #[test]
    fn test_determine_blob_next_status() {
        // When both are unpublished
        let (commit_status, reveal_status) = (L1TxStatus::Unpublished, L1TxStatus::Unpublished);
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::Unpublished);

        // When both are Finalized
        let fin = L1TxStatus::Finalized { confirmations: 5 };
        let (commit_status, reveal_status) = (fin.clone(), fin);
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::Finalized);

        // When both are Confirmed
        let conf = L1TxStatus::Confirmed { confirmations: 5 };
        let (commit_status, reveal_status) = (conf.clone(), conf.clone());
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::Confirmed);

        // When both are Published
        let publ = L1TxStatus::Published;
        let (commit_status, reveal_status) = (publ.clone(), publ.clone());
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::Published);

        // When both have invalid
        let (commit_status, reveal_status) = (L1TxStatus::InvalidInputs, L1TxStatus::InvalidInputs);
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::NeedsResign);

        // When reveal has invalid inputs but commit is confirmed. I doubt this would happen in
        // practice for our case.
        // Then the blob status should be NeedsResign i.e. the blob should be signed again and
        // published.
        let (commit_status, reveal_status) = (conf.clone(), L1TxStatus::InvalidInputs);
        let next = determine_blob_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BlobL1Status::NeedsResign);
    }
}
