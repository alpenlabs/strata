use std::{collections::HashMap, sync::Arc, time::Duration};

use bitcoin::{hashes::Hash, Txid};
use tracing::*;

use alpen_express_db::types::{ExcludeReason, L1TxEntry, L1TxStatus};

use crate::{
    broadcaster::{error::BroadcasterError, state::BroadcasterState},
    rpc::{
        traits::{L1Client, SeqL1Client},
        ClientError,
    },
};

use super::{error::BroadcasterResult, manager::BroadcastManager};

// TODO: make these configurable, possibly get from Params
const BROADCAST_POLL_INTERVAL: u64 = 1000; // millis
const FINALITY_DEPTH: u64 = 6;

/// Broadcasts the next blob to be sent
pub async fn broadcaster_task(
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    manager: Arc<BroadcastManager>,
) -> BroadcasterResult<()> {
    info!("Starting Broadcaster task");
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    let mut state = BroadcasterState::initialize(manager.clone()).await?;
    // Run indefinitely to watch/publish txs
    loop {
        interval.as_mut().tick().await;

        let (updated_entries, to_remove) =
            process_unfinalized_entries(&state.unfinalized_entries, manager.clone(), &rpc_client)
                .await
                .map_err(|e| {
                    error!(%e, "broadcaster exiting");
                    e
                })?;

        for idx in to_remove {
            _ = state.unfinalized_entries.remove(&idx);
        }

        state = state.next_state(updated_entries, manager.clone()).await?;
    }
}

/// Processes unfinalized entries and returns entries idxs that are finalized
async fn process_unfinalized_entries(
    unfinalized_entries: &HashMap<u64, L1TxEntry>,
    manager: Arc<BroadcastManager>,
    rpc_client: &Arc<impl SeqL1Client + L1Client>,
) -> BroadcasterResult<(HashMap<u64, L1TxEntry>, Vec<u64>)> {
    let mut to_remove = Vec::new();
    let mut updated_entries = HashMap::new();

    for (idx, txentry) in unfinalized_entries.iter() {
        let updated_status = handle_entry(rpc_client, txentry).await?;

        if let Some(status) = updated_status {
            let mut new_txentry = txentry.clone();
            new_txentry.status = status.clone();

            // update in db, maybe this should be moved out of this fn to separate concerns??
            manager.put_txentry_async(*idx, new_txentry.clone()).await?;

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
            let txid = Txid::from_slice(txentry.txid())
                .map_err(|e| BroadcasterError::Other(e.to_string()))?;
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
                c if c >= FINALITY_DEPTH => Ok(Some(L1TxStatus::Finalized)),
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

#[cfg(test)]
mod test {
    use alpen_express_db::{traits::TxBroadcastDatabase, types::ExcludeReason};
    use alpen_express_rocksdb::broadcaster::db::{BroadcastDatabase, BroadcastDb};
    use alpen_test_utils::ArbitraryGenerator;

    use crate::broadcaster::manager::BroadcastManager;
    use crate::test_utils::TestBitcoinClient;

    use super::*;

    fn get_db() -> Arc<impl TxBroadcastDatabase> {
        let db = alpen_test_utils::get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(BroadcastDb::new(db));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_manager() -> Arc<BroadcastManager> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let mgr = BroadcastManager::new(db, Arc::new(pool));
        Arc::new(mgr)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let arb = ArbitraryGenerator::new();
        let mut entry: L1TxEntry = arb.generate();
        entry.status = st;
        entry
    }

    #[tokio::test]
    async fn test_handle_unpublished_entry() {
        let mgr = get_manager();
        let e = gen_entry_with_status(L1TxStatus::Unpublished);

        // Add tx to db
        mgr.add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Published),
            "Status should be if tx is published"
        );
    }

    #[tokio::test]
    async fn test_handle_published_entry() {
        let mgr = get_manager();
        let e = gen_entry_with_status(L1TxStatus::Published);

        // Add tx to db
        mgr.add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res, None,
            "Status should not change if no confirmations for a published tx"
        );

        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(FINALITY_DEPTH - 1);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_confirmed_entry() {
        let mgr = get_manager();
        let e = gen_entry_with_status(L1TxStatus::Confirmed);

        // Add tx to db
        mgr.add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Unpublished),
            "Status should revert to unpublished if previously confirmed tx has 0 confirmations"
        );

        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(FINALITY_DEPTH - 1);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_finalized_entry() {
        let mgr = get_manager();
        let e = gen_entry_with_status(L1TxStatus::Finalized);

        // Add tx to db
        mgr.add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res, None,
            "Status should not change for finalized tx. Should remain the same."
        );

        // This client will return confirmations to be 0
        // NOTE: this should not occur in practice though
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res, None,
            "Status should not change for finalized tx. Should remain the same."
        );
    }

    #[tokio::test]
    async fn test_handle_excluded_entry() {
        let mgr = get_manager();
        let e = gen_entry_with_status(L1TxStatus::Excluded(ExcludeReason::Other(
            "some reason".to_string(),
        )));

        // Add tx to db
        mgr.add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res, None,
            "Status should not change for excluded tx. Should remain the same."
        );

        // This client will return confirmations to be 0
        // NOTE: this should not occur in practice for a finalized tx though
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(&cl, &e).await.unwrap();
        assert_eq!(
            res, None,
            "Status should not change for excluded tx. Should remain the same."
        );
    }

    #[tokio::test]
    async fn test_process_unfinalized_entries() {
        let mgr = get_manager();
        // Add a couple of txs
        let e1 = gen_entry_with_status(L1TxStatus::Unpublished);
        let i1 = mgr
            .add_txentry_async((*e1.txid()).into(), e1)
            .await
            .unwrap();
        let e2 = gen_entry_with_status(L1TxStatus::Excluded(ExcludeReason::MissingInputsOrSpent));
        let _i2 = mgr
            .add_txentry_async((*e2.txid()).into(), e2)
            .await
            .unwrap();

        let e3 = gen_entry_with_status(L1TxStatus::Published);
        let i3 = mgr
            .add_txentry_async((*e3.txid()).into(), e3)
            .await
            .unwrap();

        let state = BroadcasterState::initialize(mgr.clone()).await.unwrap();

        // This client will make the published tx finalized
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let (new_entries, to_remove) =
            process_unfinalized_entries(&state.unfinalized_entries, mgr, &cl)
                .await
                .unwrap();

        // The published tx which got finalized should be removed
        assert_eq!(
            to_remove,
            vec![i3],
            "Finalized tx should be in to_remove list"
        );

        assert_eq!(
            new_entries.get(&i1).unwrap().status,
            L1TxStatus::Published,
            "unpublished tx should be published"
        );
        assert_eq!(
            new_entries.get(&i3).unwrap().status,
            L1TxStatus::Finalized,
            "published tx should be finalized"
        );
    }
}
