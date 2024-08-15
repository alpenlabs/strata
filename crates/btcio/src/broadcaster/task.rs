use std::{collections::HashMap, sync::Arc, time::Duration};

use bitcoin::{hashes::Hash, Txid};
use tracing::*;

use alpen_express_db::{
    traits::{BcastStore, TxBroadcastDatabase},
    types::{ExcludeReason, L1TxEntry, L1TxStatus},
};

use crate::{
    broadcaster::{error::BroadcasterError, state::BroadcasterState},
    rpc::{
        traits::{L1Client, SeqL1Client},
        ClientError,
    },
};

use super::error::BroadcasterResult;

// TODO: make these configurable, possibly get from Params
const BROADCAST_POLL_INTERVAL: u64 = 1000; // millis
const FINALITY_DEPTH: u64 = 6;

/// Broadcasts the next blob to be sent
// TODO: make use of broadcast manager instead of db
pub async fn broadcaster_task<D: TxBroadcastDatabase + Send + Sync + 'static>(
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    db: Arc<D>,
) -> BroadcasterResult<()> {
    info!("Starting Broadcaster task");
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    let mut state = BroadcasterState::from_db(db.clone())?;
    // Run indefinitely to watch/publish txs
    loop {
        interval.as_mut().tick().await;

        let (updated_entries, to_remove) =
            process_unfinalized_entries(&state.unfinalized_entries, db.clone(), &rpc_client)
                .await
                .map_err(|e| {
                    error!(%e, "broadcaster exiting");
                    e
                })?;

        for idx in to_remove {
            _ = state.unfinalized_entries.remove(&idx);
        }

        let temp_state = BroadcasterState::from_db_start_idx(db.clone(), state.next_idx)?;
        if temp_state.next_idx < state.next_idx {
            return Err(BroadcasterError::Other(
                "Inconsistent db idx and state idx".to_string(),
            ));
        }
        // Update state
        state.unfinalized_entries = updated_entries;
        state
            .unfinalized_entries
            .extend(temp_state.unfinalized_entries);
        state.next_idx = temp_state.next_idx;
    }
}

/// Processes unfinalized entries and returns entries idxs that are finalized
async fn process_unfinalized_entries<D: TxBroadcastDatabase + Send + Sync + 'static>(
    unfinalized_entries: &HashMap<u64, L1TxEntry>,
    db: Arc<D>,
    rpc_client: &Arc<impl SeqL1Client + L1Client>,
) -> BroadcasterResult<(HashMap<u64, L1TxEntry>, Vec<u64>)> {
    let mut to_remove = Vec::new();
    let mut updated_entries = HashMap::new();
    for (idx, txentry) in unfinalized_entries.iter() {
        let updated_status = handle_entry(rpc_client, txentry).await?;

        if let Some(status) = updated_status {
            let mut new_txentry = txentry.clone();
            new_txentry.status = status.clone();
            // update in db
            db.broadcast_store()
                .update_tx_by_idx(*idx, new_txentry.clone())?;

            // Remove if finalized
            if status == L1TxStatus::Finalized {
                to_remove.push(*idx);
            }

            updated_entries.insert(*idx, new_txentry);
        } else {
            updated_entries.insert(*idx, txentry.clone());
        }
    }
    Ok((updated_entries, to_remove))
}

/// Takes in `[L1TxEntry]`, checks status and then either publishes or checks for confirmations and
/// returns its updated status. Returns None if status is not changed
async fn handle_entry(
    rpc_client: &Arc<impl SeqL1Client + L1Client>,
    txentry: &L1TxEntry,
) -> BroadcasterResult<Option<L1TxStatus>> {
    match txentry.status {
        L1TxStatus::Unpublished => {
            // Try to publish
            match send_tx(txentry.tx_raw(), rpc_client).await {
                Ok(_) => Ok(Some(L1TxStatus::Published)),
                Err(PublishError::MissingInputsOrSpent) => {
                    warn!(
                        ?txentry,
                        "tx exculded while broadcasting due to missing or spent inputs"
                    );
                    Ok(Some(L1TxStatus::Excluded(
                        ExcludeReason::MissingInputsOrSpent,
                    )))
                }
                Err(PublishError::Other(str)) => {
                    warn!(?txentry, %str, "tx excluded while broadcasting");
                    Err(BroadcasterError::Other(str))
                }
            }
        }
        L1TxStatus::Published | L1TxStatus::Confirmed => {
            // check for confirmations
            let txid = Txid::from_slice(txentry.txid()).map_err(|e| BroadcasterError::Other(e.to_string()))?;
            match rpc_client
                .get_transaction_confirmations(txid)
                .await
                .map_err(|e| BroadcasterError::Other(e.to_string()))?
            {
                0 if txentry.status == L1TxStatus::Confirmed => {
                    // if the confirmations of a txn that is already confirmed is 0 then there is
                    // something wrong, possibly a reorg, so just set it to unpublished
                    Ok(Some(L1TxStatus::Unpublished))
                }
                0 => Ok(None),
                c if c >= FINALITY_DEPTH => Ok(Some(L1TxStatus::Confirmed)),
                _ => Ok(Some(L1TxStatus::Confirmed)),
            }
        }
        L1TxStatus::Finalized => Ok(None), // Nothing to do for finalized tx
        L1TxStatus::Excluded(_) => {
            // If a tx is excluded due to MissingInputsOrSpent then the downstream task like
            // writer/signer will be accountable for recreating the tx and asking to broadcast.
            // If excluded due to Other reason, there's nothing much we can do.
            Ok(None)
        }
    }
}

#[derive(Debug)]
enum PublishError {
    MissingInputsOrSpent,
    Other(String),
}

async fn send_tx(
    tx_raw: &[u8],
    client: &Arc<impl SeqL1Client + L1Client>,
) -> Result<(), PublishError> {
    match client.send_raw_transaction(tx_raw).await {
        Ok(_) => Ok(()),
        Err(ClientError::Server(-27, _)) => Ok(()), // Tx already included
        Err(ClientError::Server(-25, _)) => Err(PublishError::MissingInputsOrSpent),
        Err(e) => Err(PublishError::Other(e.to_string())),
    }
}
