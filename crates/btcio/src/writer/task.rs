use std::{sync::Arc, time::Duration};

use bitcoin::Address;
use strata_config::btcio::WriterConfig;
use strata_db::{
    traits::SequencerDatabase,
    types::{IntentEntry, L1TxStatus, PayloadEntry, PayloadL1Status},
};
use strata_primitives::{
    l1::payload::{PayloadDest, PayloadIntent},
    params::Params,
};
use strata_status::StatusChannel;
use strata_storage::ops::envelope::{Context, EnvelopeDataOps};
use strata_tasks::TaskExecutor;
use tracing::*;

use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::{traits::WriterRpc, BitcoinClient},
    status::{apply_status_updates, L1StatusUpdate},
    writer::{
        builder::EnvelopeError, context::WriterContext, signer::create_and_sign_payload_envelopes,
    },
};

/// A handle to the Envelope task.
pub struct EnvelopeHandle {
    ops: Arc<EnvelopeDataOps>,
}

impl EnvelopeHandle {
    pub fn new(ops: Arc<EnvelopeDataOps>) -> Self {
        Self { ops }
    }

    pub fn submit_intent(&self, intent: PayloadIntent) -> anyhow::Result<()> {
        if intent.dest() != PayloadDest::L1 {
            warn!(commitment = %intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let id = *intent.commitment();
        debug!(commitment = %intent.commitment(), "Received intent");
        if self.ops.get_intent_by_id_blocking(id)?.is_some() {
            warn!(commitment = %id, "Received duplicate intent");
            return Ok(());
        }
        let entry = IntentEntry::new_unbundled(intent);

        Ok(self.ops.put_intent_entry_blocking(id, entry)?)
    }

    pub async fn submit_intent_async(&self, intent: PayloadIntent) -> anyhow::Result<()> {
        if intent.dest() != PayloadDest::L1 {
            warn!(commitment = %intent.commitment(), "Received intent not meant for L1");
            return Ok(());
        }

        let id = *intent.commitment();
        debug!(commitment = %intent.commitment(), "Received intent");
        if self.ops.get_intent_by_id_async(id).await?.is_some() {
            warn!(commitment = %id, "Received duplicate intent");
            return Ok(());
        }
        let entry = IntentEntry::new_unbundled(intent);

        Ok(self.ops.put_intent_entry_async(id, entry).await?)
    }
}

/// Starts the envelope task.
///
/// This creates an [`EnvelopeHandle`] and spawns a watcher task that watches the status of
/// incriptions in bitcoin.
///
/// # Returns
///
/// [`Result<EnvelopeHandle>`](anyhow::Result)
#[allow(clippy::too_many_arguments)]
pub fn start_envelope_task<D: SequencerDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    bitcoin_client: Arc<BitcoinClient>,
    config: Arc<WriterConfig>,
    params: Arc<Params>,
    sequencer_address: Address,
    db: Arc<D>,
    status_channel: StatusChannel,
    pool: threadpool::ThreadPool,
    broadcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<Arc<EnvelopeHandle>> {
    let envelope_data_ops = Arc::new(Context::new(db).into_ops(pool));
    let next_watch_payload_idx = get_next_payloadidx_to_watch(envelope_data_ops.as_ref())?;

    let envelope_handle = Arc::new(EnvelopeHandle::new(envelope_data_ops.clone()));
    let ctx = Arc::new(WriterContext::new(
        params,
        config,
        sequencer_address,
        bitcoin_client,
        status_channel,
    ));

    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(
            next_watch_payload_idx,
            ctx,
            envelope_data_ops,
            broadcast_handle,
        )
        .await
    });

    Ok(envelope_handle)
}

/// Looks into the database from descending index order till it reaches 0 or `Finalized`
/// [`PayloadEntry`] from which the rest of the [`PayloadEntry`]s should be watched.
fn get_next_payloadidx_to_watch(insc_ops: &EnvelopeDataOps) -> anyhow::Result<u64> {
    let mut next_idx = insc_ops.get_next_payload_idx_blocking()?;

    while next_idx > 0 {
        let Some(payload) = insc_ops.get_payload_entry_by_idx_blocking(next_idx - 1)? else {
            break;
        };
        if payload.status == PayloadL1Status::Finalized {
            break;
        };
        next_idx -= 1;
    }
    Ok(next_idx)
}

/// Watches for envelope transactions status in bitcoin. Note that this watches for each
/// envelope until it is confirmed
/// Watches for envelope transactions status in the Bitcoin blockchain.
///
/// # Note
///
/// The envelope will be monitored until it acquires the status of
/// [`BlobL1Status::Finalized`]
pub async fn watcher_task<W: WriterRpc>(
    next_blbidx_to_watch: u64,
    context: Arc<WriterContext<W>>,
    insc_ops: Arc<EnvelopeDataOps>,
    broadcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's watcher task");
    let interval = tokio::time::interval(Duration::from_millis(context.config.write_poll_dur_ms));
    tokio::pin!(interval);

    let mut curr_payloadidx = next_blbidx_to_watch;
    loop {
        interval.as_mut().tick().await;

        if let Some(payloadentry) = insc_ops
            .get_payload_entry_by_idx_async(curr_payloadidx)
            .await?
        {
            match payloadentry.status {
                // If unsigned or needs resign, create new signed commit/reveal txs and update the
                // entry
                PayloadL1Status::Unsigned | PayloadL1Status::NeedsResign => {
                    debug!(?payloadentry.status, %curr_payloadidx, "Processing unsigned payloadentry");
                    match create_and_sign_payload_envelopes(
                        &payloadentry,
                        &broadcast_handle,
                        context.clone(),
                    )
                    .await
                    {
                        Ok((cid, rid)) => {
                            let mut updated_entry = payloadentry.clone();
                            updated_entry.status = PayloadL1Status::Unpublished;
                            updated_entry.commit_txid = cid;
                            updated_entry.reveal_txid = rid;
                            update_existing_entry(curr_payloadidx, updated_entry, &insc_ops)
                                .await?;

                            debug!(%curr_payloadidx, "Signed payload");
                        }
                        Err(EnvelopeError::NotEnoughUtxos(required, available)) => {
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
                PayloadL1Status::Finalized => {
                    curr_payloadidx += 1;
                }
                // If entry is signed but not finalized or excluded yet, check broadcast txs status
                PayloadL1Status::Published
                | PayloadL1Status::Confirmed
                | PayloadL1Status::Unpublished => {
                    debug!(%curr_payloadidx, "Checking payloadentry's broadcast status");
                    let commit_tx = broadcast_handle
                        .get_tx_entry_by_id_async(payloadentry.commit_txid)
                        .await?;
                    let reveal_tx = broadcast_handle
                        .get_tx_entry_by_id_async(payloadentry.reveal_txid)
                        .await?;

                    match (commit_tx, reveal_tx) {
                        (Some(ctx), Some(rtx)) => {
                            let new_status =
                                determine_payload_next_status(&ctx.status, &rtx.status);
                            debug!(?new_status, "The next status for payload");

                            update_l1_status(&payloadentry, &new_status, &context.status_channel)
                                .await;

                            // Update payloadentry with new status
                            let mut updated_entry = payloadentry.clone();
                            updated_entry.status = new_status.clone();
                            update_existing_entry(curr_payloadidx, updated_entry, &insc_ops)
                                .await?;

                            if new_status == PayloadL1Status::Finalized {
                                curr_payloadidx += 1;
                            }
                        }
                        _ => {
                            warn!(%curr_payloadidx, "Corresponding commit/reveal entry for payloadentry not found in broadcast db. Sign and create transactions again.");
                            let mut updated_entry = payloadentry.clone();
                            updated_entry.status = PayloadL1Status::Unsigned;
                            update_existing_entry(curr_payloadidx, updated_entry, &insc_ops)
                                .await?;
                        }
                    }
                }
            }
        } else {
            // No payload exists, just continue the loop to wait for payload's presence in db
            info!(%curr_payloadidx, "Waiting for payloadentry to be present in db");
        }
    }
}

async fn update_l1_status(
    payloadentry: &PayloadEntry,
    new_status: &PayloadL1Status,
    status_channel: &StatusChannel,
) {
    // Update L1 status. Since we are processing one payloadentry at a time, if the entry is
    // finalized/confirmed, then it means it is published as well
    if *new_status == PayloadL1Status::Published
        || *new_status == PayloadL1Status::Confirmed
        || *new_status == PayloadL1Status::Finalized
    {
        let status_updates = [
            L1StatusUpdate::LastPublishedTxid(payloadentry.reveal_txid.into()),
            L1StatusUpdate::IncrementPublishedRevealCount,
        ];
        apply_status_updates(&status_updates, status_channel).await;
    }
}

async fn update_existing_entry(
    idx: u64,
    updated_entry: PayloadEntry,
    insc_ops: &EnvelopeDataOps,
) -> anyhow::Result<()> {
    let msg = format!("Expect to find payloadentry {idx} in db");
    let id = insc_ops.get_payload_entry_id_async(idx).await?.expect(&msg);
    Ok(insc_ops.put_payload_entry_async(id, updated_entry).await?)
}

/// Determine the status of the `PayloadEntry` based on the status of its commit and reveal
/// transactions in bitcoin.
fn determine_payload_next_status(
    commit_status: &L1TxStatus,
    reveal_status: &L1TxStatus,
) -> PayloadL1Status {
    match (&commit_status, &reveal_status) {
        // If reveal is finalized, both are finalized
        (_, L1TxStatus::Finalized { .. }) => PayloadL1Status::Finalized,
        // If reveal is confirmed, both are confirmed
        (_, L1TxStatus::Confirmed { .. }) => PayloadL1Status::Confirmed,
        // If reveal is published regardless of commit, the payload is published
        (_, L1TxStatus::Published) => PayloadL1Status::Published,
        // if commit has invalid inputs, needs resign
        (L1TxStatus::InvalidInputs, _) => PayloadL1Status::NeedsResign,
        // If commit is unpublished, both are upublished
        (L1TxStatus::Unpublished, _) => PayloadL1Status::Unpublished,
        // If commit is published but not reveal, the payload is unpublished
        (_, L1TxStatus::Unpublished) => PayloadL1Status::Unpublished,
        // If reveal has invalid inputs, these need resign because we can do nothing with just
        // commit tx confirmed. This should not occur in practice
        (_, L1TxStatus::InvalidInputs) => PayloadL1Status::NeedsResign,
    }
}

#[cfg(test)]
mod test {
    use strata_primitives::buf::Buf32;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::writer::test_utils::get_envelope_ops;

    #[test]
    fn test_initialize_writer_state_no_last_payload_idx() {
        let iops = get_envelope_ops();

        let nextidx = iops.get_next_payload_idx_blocking().unwrap();
        assert_eq!(nextidx, 0);

        let idx = get_next_payloadidx_to_watch(&iops).unwrap();

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_payloads() {
        let iops = get_envelope_ops();

        let mut e1: PayloadEntry = ArbitraryGenerator::new().generate();
        e1.status = PayloadL1Status::Finalized;
        let payload_hash: Buf32 = [1; 32].into();
        iops.put_payload_entry_blocking(payload_hash, e1).unwrap();
        let expected_idx = iops.get_next_payload_idx_blocking().unwrap();

        let mut e2: PayloadEntry = ArbitraryGenerator::new().generate();
        e2.status = PayloadL1Status::Published;
        let payload_hash: Buf32 = [2; 32].into();
        iops.put_payload_entry_blocking(payload_hash, e2).unwrap();

        let mut e3: PayloadEntry = ArbitraryGenerator::new().generate();
        e3.status = PayloadL1Status::Unsigned;
        let payload_hash: Buf32 = [3; 32].into();
        iops.put_payload_entry_blocking(payload_hash, e3).unwrap();

        let mut e4: PayloadEntry = ArbitraryGenerator::new().generate();
        e4.status = PayloadL1Status::Unsigned;
        let payload_hash: Buf32 = [4; 32].into();
        iops.put_payload_entry_blocking(payload_hash, e4).unwrap();

        let idx = get_next_payloadidx_to_watch(&iops).unwrap();

        assert_eq!(idx, expected_idx);
    }

    #[test]
    fn test_determine_payload_next_status() {
        // When both are unpublished
        let (commit_status, reveal_status) = (L1TxStatus::Unpublished, L1TxStatus::Unpublished);
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::Unpublished);

        // When both are Finalized
        let fin = L1TxStatus::Finalized { confirmations: 5 };
        let (commit_status, reveal_status) = (fin.clone(), fin);
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::Finalized);

        // When both are Confirmed
        let conf = L1TxStatus::Confirmed { confirmations: 5 };
        let (commit_status, reveal_status) = (conf.clone(), conf.clone());
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::Confirmed);

        // When both are Published
        let publ = L1TxStatus::Published;
        let (commit_status, reveal_status) = (publ.clone(), publ.clone());
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::Published);

        // When both have invalid
        let (commit_status, reveal_status) = (L1TxStatus::InvalidInputs, L1TxStatus::InvalidInputs);
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::NeedsResign);

        // When reveal has invalid inputs but commit is confirmed. I doubt this would happen in
        // practice for our case.
        // Then the payload status should be NeedsResign i.e. the payload should be signed again and
        // published.
        let (commit_status, reveal_status) = (conf.clone(), L1TxStatus::InvalidInputs);
        let next = determine_payload_next_status(&commit_status, &reveal_status);
        assert_eq!(next, PayloadL1Status::NeedsResign);
    }
}
