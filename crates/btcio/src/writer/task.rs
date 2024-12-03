use std::{sync::Arc, time::Duration};

use strata_btcio_rpc_types::traits::{Reader, Signer, Wallet};
use strata_btcio_tx::reveal::builder::CommitRevealTxError;
use strata_db::{
    traits::WriterDatabase,
    types::{BundleL1Status, DataBundleIntentEntry, L1TxStatus},
};
use strata_primitives::buf::Buf32;
use strata_state::da_blob::{DataBundleDest, PayloadIntent};
use strata_status::StatusChannel;
use strata_storage::ops::envelope::{Context, EnvelopeDataOps};
use strata_tasks::TaskExecutor;
use tracing::*;

use super::config::WriterConfig;
use crate::{
    broadcaster::L1BroadcastHandle,
    status::{apply_status_updates, L1StatusUpdate},
    writer::signer::create_and_sign_commit_reveal_txs,
};

/// A handle to the envelope task which gets published as commit reveal txs.
pub struct EnvelopeHandle {
    ops: Arc<EnvelopeDataOps>,
}

impl EnvelopeHandle {
    pub fn new(ops: Arc<EnvelopeDataOps>) -> Self {
        Self { ops }
    }

    pub fn submit_intent(&self, intent: PayloadIntent) -> anyhow::Result<()> {
        if intent.dest() != DataBundleDest::L1 {
            warn!(commitment = ?intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let entry = DataBundleIntentEntry::new_unsigned(vec![intent.payload().clone()]);
        self.submit_entry_sync(intent.commitment().into_inner(), entry)
    }

    pub async fn submit_intent_async(&self, intent: PayloadIntent) -> anyhow::Result<()> {
        if intent.dest() != DataBundleDest::L1 {
            warn!(commitment = ?intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let entry = DataBundleIntentEntry::new_unsigned(vec![intent.payload().clone()]);
        self.submit_entry_async(intent.commitment().into_inner(), entry)
            .await
    }

    fn submit_entry_sync(
        &self,
        commitment: Buf32,
        entry: DataBundleIntentEntry,
    ) -> anyhow::Result<()> {
        debug!(?commitment, "Received intent");
        if self.ops.get_entry_blocking(commitment)?.is_some() {
            warn!(?commitment, "Received duplicate intent");
            return Ok(());
        }
        self.ops.put_entry_blocking(commitment, entry)?;
        Ok(())
    }

    async fn submit_entry_async(
        &self,
        commitment: Buf32,
        entry: DataBundleIntentEntry,
    ) -> anyhow::Result<()> {
        debug!(?commitment, "Received intent");
        if self.ops.get_entry_async(commitment).await?.is_some() {
            warn!(?commitment, "Received duplicate intent");
            return Ok(());
        }
        self.ops.put_entry_async(commitment, entry).await?;
        Ok(())
    }
}

/// Starts the Envelope task, which ultimately will be placed as a commit reveal transaction
///
/// This creates an [`EnvelopeHandle`] and spawns a watcher task that watches the status of
/// envelopes in bitcoin.
///
/// # Returns
///
/// [`Result<EnvelopeHandle>`](anyhow::Result)
pub fn start_envelope_task<D: WriterDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    bitcoin_client: Arc<impl Reader + Wallet + Signer + Send + Sync + 'static>,
    config: WriterConfig,
    db: Arc<D>,
    status_channel: StatusChannel,
    pool: threadpool::ThreadPool,
    broadcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<Arc<EnvelopeHandle>> {
    let envelope_data_ops = Arc::new(Context::new(db).into_ops(pool));
    let next_watch_entry_idx = get_next_entry_idx_to_watch(envelope_data_ops.as_ref())?;

    let envelope_handle = Arc::new(EnvelopeHandle::new(envelope_data_ops.clone()));

    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(
            next_watch_entry_idx,
            bitcoin_client,
            config,
            envelope_data_ops,
            broadcast_handle,
            status_channel,
        )
        .await
    });

    Ok(envelope_handle)
}

/// Looks into the database from descending index order till it reaches 0 or `Finalized`
/// [`DataBundleIntentEntry`] from which the rest of the [`DataBundleIntentEntry`]s should be
/// watched.
fn get_next_entry_idx_to_watch(insc_ops: &EnvelopeDataOps) -> anyhow::Result<u64> {
    let mut next_idx = insc_ops.get_next_entry_idx_blocking()?;

    while next_idx > 0 {
        let Some(entry) = insc_ops.get_entry_by_idx_blocking(next_idx - 1)? else {
            break;
        };
        if entry.status == BundleL1Status::Finalized {
            break;
        };
        next_idx -= 1;
    }
    Ok(next_idx)
}

/// Watches for commit reveal transactions status in bitcoin. The transaction will
/// be monitored until it acquires the status of [`PayloadL1Status::Finalized`]
pub async fn watcher_task(
    next_entry_to_watch: u64,
    bitcoin_client: Arc<impl Reader + Wallet + Signer>,
    config: WriterConfig,
    envelope_ops: Arc<EnvelopeDataOps>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    status_channel: StatusChannel,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's watcher task");
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    let mut curr_entry_idx = next_entry_to_watch;
    loop {
        interval.as_mut().tick().await;

        if let Some(entry) = envelope_ops.get_entry_by_idx_async(curr_entry_idx).await? {
            match entry.status {
                // If unsigned or needs resign, create new signed commit/reveal txs and update the
                // entry
                BundleL1Status::Unsigned | BundleL1Status::NeedsResign => {
                    debug!(?entry.status, %curr_entry_idx, "Processing unsigned entry");
                    match create_and_sign_commit_reveal_txs(
                        &entry,
                        &broadcast_handle,
                        bitcoin_client.clone(),
                        &config,
                    )
                    .await
                    {
                        Ok((cid, rid)) => {
                            let mut updated_entry = entry.clone();
                            updated_entry.status = BundleL1Status::Unpublished;
                            updated_entry.commit_txid = cid;
                            updated_entry.reveal_txid = rid;
                            update_existing_entry(curr_entry_idx, updated_entry, &envelope_ops)
                                .await?;

                            debug!(%curr_entry_idx, "Signed entry");
                        }
                        Err(CommitRevealTxError::NotEnoughUtxos(required, available)) => {
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
                BundleL1Status::Finalized => {
                    curr_entry_idx += 1;
                }
                // If entry is signed but not finalized or excluded yet, check broadcast txs status
                BundleL1Status::Published
                | BundleL1Status::Confirmed
                | BundleL1Status::Unpublished => {
                    debug!(%curr_entry_idx, "Checking entry's broadcast status");
                    let commit_tx = broadcast_handle
                        .get_tx_entry_by_id_async(entry.commit_txid)
                        .await?;
                    let reveal_tx = broadcast_handle
                        .get_tx_entry_by_id_async(entry.reveal_txid)
                        .await?;

                    match (commit_tx, reveal_tx) {
                        (Some(ctx), Some(rtx)) => {
                            let new_status =
                                determine_envelope_entry_next_status(&ctx.status, &rtx.status);
                            debug!(?new_status, "The next status for entry");

                            update_l1_status(&entry, &new_status, &status_channel).await;

                            // Update entry with new status
                            let mut updated_entry = entry.clone();
                            updated_entry.status = new_status.clone();
                            update_existing_entry(curr_entry_idx, updated_entry, &envelope_ops)
                                .await?;

                            if new_status == BundleL1Status::Finalized {
                                curr_entry_idx += 1;
                            }
                        }
                        _ => {
                            warn!(%curr_entry_idx, "Corresponding commit/reveal entry for entry not found in broadcast db. Sign and create transactions again.");
                            let mut updated_entry = entry.clone();
                            updated_entry.status = BundleL1Status::Unsigned;
                            update_existing_entry(curr_entry_idx, updated_entry, &envelope_ops)
                                .await?;
                        }
                    }
                }
            }
        } else {
            // No entry exists, just continue the loop to wait for entry presence in db
            info!(%curr_entry_idx, "Waiting for entry to be present in db");
        }
    }
}

async fn update_l1_status(
    entry: &DataBundleIntentEntry,
    new_status: &BundleL1Status,
    status_channel: &StatusChannel,
) {
    // Update L1 status. Since we are processing one entry at a time, if the entry is
    // finalized/confirmed, then it means it is published as well
    if *new_status == BundleL1Status::Published
        || *new_status == BundleL1Status::Confirmed
        || *new_status == BundleL1Status::Finalized
    {
        let status_updates = [
            L1StatusUpdate::LastPublishedTxid(entry.reveal_txid.into()),
            L1StatusUpdate::IncrementCommitRevealTxCount,
        ];
        apply_status_updates(&status_updates, status_channel).await;
    }
}

async fn update_existing_entry(
    idx: u64,
    updated_entry: DataBundleIntentEntry,
    envelope_ops: &EnvelopeDataOps,
) -> anyhow::Result<()> {
    let msg = format!("Expect to find entry {idx} in db");
    let id = envelope_ops.get_entry_id_async(idx).await?.expect(&msg);
    Ok(envelope_ops.put_entry_async(id, updated_entry).await?)
}

/// Determine the status of the [`DataBundleIntentEntry`] based on the status of its commit and
/// reveal transactions in bitcoin.
fn determine_envelope_entry_next_status(
    commit_status: &L1TxStatus,
    reveal_status: &L1TxStatus,
) -> BundleL1Status {
    match (&commit_status, &reveal_status) {
        // If reveal is finalized, both are finalized
        (_, L1TxStatus::Finalized { .. }) => BundleL1Status::Finalized,
        // If reveal is confirmed, both are confirmed
        (_, L1TxStatus::Confirmed { .. }) => BundleL1Status::Confirmed,
        // If reveal is published regardless of commit, the envelope is published
        (_, L1TxStatus::Published) => BundleL1Status::Published,
        // if commit has invalid inputs, needs resign
        (L1TxStatus::InvalidInputs, _) => BundleL1Status::NeedsResign,
        // If commit is unpublished, both are upublished
        (L1TxStatus::Unpublished, _) => BundleL1Status::Unpublished,
        // If commit is published but not reveal, the entry is unpublished
        (_, L1TxStatus::Unpublished) => BundleL1Status::Unpublished,
        // If reveal has invalid inputs, these need resign because we can do nothing with just
        // commit tx confirmed. This should not occur in practice
        (_, L1TxStatus::InvalidInputs) => BundleL1Status::NeedsResign,
    }
}

#[cfg(test)]
mod test {
    use strata_primitives::buf::Buf32;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::writer::test_utils::get_envelope_ops;

    #[test]
    fn test_initialize_writer_state_no_last_entry_idx() {
        let iops = get_envelope_ops();

        let nextidx = iops.get_next_entry_idx_blocking().unwrap();
        assert_eq!(nextidx, 0);

        let idx = get_next_entry_idx_to_watch(&iops).unwrap();

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_envelopes() {
        let iops = get_envelope_ops();

        let mut e1: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        e1.status = BundleL1Status::Finalized;
        let hash: Buf32 = [1; 32].into();
        iops.put_entry_blocking(hash, e1).unwrap();
        let expected_idx = iops.get_next_entry_idx_blocking().unwrap();

        let mut e2: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        e2.status = BundleL1Status::Published;
        let hash: Buf32 = [2; 32].into();
        iops.put_entry_blocking(hash, e2).unwrap();

        let mut e3: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        e3.status = BundleL1Status::Unsigned;
        let hash: Buf32 = [3; 32].into();
        iops.put_entry_blocking(hash, e3).unwrap();

        let mut e4: DataBundleIntentEntry = ArbitraryGenerator::new().generate();
        e4.status = BundleL1Status::Unsigned;
        let hash: Buf32 = [4; 32].into();
        iops.put_entry_blocking(hash, e4).unwrap();

        let idx = get_next_entry_idx_to_watch(&iops).unwrap();

        assert_eq!(idx, expected_idx);
    }

    #[test]
    fn test_determine_entry_next_status() {
        // When both are unpublished
        let (commit_status, reveal_status) = (L1TxStatus::Unpublished, L1TxStatus::Unpublished);
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::Unpublished);

        // When both are Finalized
        let fin = L1TxStatus::Finalized { confirmations: 5 };
        let (commit_status, reveal_status) = (fin.clone(), fin);
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::Finalized);

        // When both are Confirmed
        let conf = L1TxStatus::Confirmed { confirmations: 5 };
        let (commit_status, reveal_status) = (conf.clone(), conf.clone());
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::Confirmed);

        // When both are Published
        let publ = L1TxStatus::Published;
        let (commit_status, reveal_status) = (publ.clone(), publ.clone());
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::Published);

        // When both have invalid
        let (commit_status, reveal_status) = (L1TxStatus::InvalidInputs, L1TxStatus::InvalidInputs);
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::NeedsResign);

        // When reveal has invalid inputs but commit is confirmed. I doubt this would happen in
        // practice for our case.
        // Then the envelope status should be NeedsResign i.e. the envelope should be signed again
        // and published.
        let (commit_status, reveal_status) = (conf.clone(), L1TxStatus::InvalidInputs);
        let next = determine_envelope_entry_next_status(&commit_status, &reveal_status);
        assert_eq!(next, BundleL1Status::NeedsResign);
    }
}
