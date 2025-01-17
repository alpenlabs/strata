use std::{sync::Arc, time::Duration};

use strata_db::types::{IntentEntry, IntentStatus, PayloadEntry};
use strata_storage::ops::writer::EnvelopeDataOps;
use tokio::time::sleep;
use tracing::*;

// TODO: get this from config
const BUNDLE_INTERVAL: u64 = 200; // millis

/// Periodically bundles unbundled intents into payload entries.
pub(crate) async fn bundler_task(ops: Arc<EnvelopeDataOps>) -> anyhow::Result<()> {
    let mut last_idx = 0;
    loop {
        let (unbundled, new_idx) = get_unbundled_intents_after(last_idx, ops.as_ref()).await?;
        process_unbundled_entries(ops.as_ref(), unbundled).await?;
        last_idx = new_idx;

        let _ = sleep(Duration::from_millis(BUNDLE_INTERVAL)).await;
    }
}

/// Processes and bundles a list of unbundled intents into payload entries.
/// NOTE: The logic current is simply 1-1 mapping between intents and payloads, in future it can
/// be sophisticated.
async fn process_unbundled_entries(
    ops: &EnvelopeDataOps,
    unbundled: Vec<IntentEntry>,
) -> anyhow::Result<()> {
    for mut entry in unbundled {
        // NOTE: In future, the logic to create payload will be different. We need to group
        // intents and create payload entries accordingly
        let payload_entry = PayloadEntry::new_unsigned(vec![entry.payload().clone()]);

        // TODO: the following block till "Atomic Ends" should be atomic.
        let idx = ops.get_next_payload_idx_async().await?;
        ops.put_payload_entry_async(idx, payload_entry).await?;

        // Set the entry to be bundled so that it won't be processed next time.
        entry.status = IntentStatus::Bundled(idx);
        ops.put_intent_entry_async(*entry.intent.commitment(), entry)
            .await?;
        // Atomic Ends.
    }
    Ok(())
}

/// Retrieves unbundled intents after a given index in ascending order along with the latest
/// unbundled entry idx.
async fn get_unbundled_intents_after(
    idx: u64,
    ops: &EnvelopeDataOps,
) -> anyhow::Result<(Vec<IntentEntry>, u64)> {
    let latest_idx = ops.get_next_intent_idx_async().await?.saturating_sub(1);
    let mut curr_intent_idx = latest_idx;
    let mut unbundled_intents = Vec::new();
    while curr_intent_idx >= idx {
        if let Some(intent_entry) = ops.get_intent_by_idx_async(curr_intent_idx).await? {
            match intent_entry.status {
                IntentStatus::Unbundled => unbundled_intents.push(intent_entry),
                IntentStatus::Bundled(_) => {
                    // Bundled intent found, no more to scan
                    break;
                }
            }
        } else {
            warn!(%curr_intent_idx, "Could not find expected intent in db");
            break;
        }

        if curr_intent_idx == 0 {
            break;
        }
        curr_intent_idx -= 1;
    }

    // Reverse the items so that they are in ascending order of index
    unbundled_intents.reverse();

    Ok((unbundled_intents, latest_idx))
}
