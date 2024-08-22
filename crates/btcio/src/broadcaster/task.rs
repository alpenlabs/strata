use std::{collections::BTreeMap, sync::Arc, time::Duration};

use alpen_express_primitives::buf::Buf32;
use bitcoin::{hashes::Hash, Txid};
use express_storage::{ops::l1tx_broadcast, BroadcastDbOps};
use tokio::sync::mpsc::Receiver;
use tracing::*;

use alpen_express_db::types::{ExcludeReason, L1TxEntry, L1TxStatus};

use crate::{
    broadcaster::{error::BroadcasterError, state::BroadcasterState},
    rpc::{
        traits::{L1Client, SeqL1Client},
        ClientError,
    },
};

use super::error::BroadcasterResult;

// TODO: make these configurable, get from config
const BROADCAST_POLL_INTERVAL: u64 = 1000; // millis
const FINALITY_DEPTH: u64 = 6;

/// Broadcasts the next blob to be sent
pub async fn broadcaster_task(
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    ops: Arc<l1tx_broadcast::BroadcastDbOps>,
    mut entry_receiver: Receiver<(u64, L1TxEntry)>,
) -> BroadcasterResult<()> {
    info!("Starting Broadcaster task");
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    let mut state = BroadcasterState::initialize(&ops).await?;

    // Run indefinitely to watch/publish txs
    loop {
        tokio::select! {
            _ = interval.tick() => {}

            Some((idx, txentry)) = entry_receiver.recv() => {
                let txid = get_txid_str(idx, ops.as_ref()).await?;
                info!(%idx, %txid, "Received txentry");

                // Insert into state's unfinalized entries. Need not update next_idx because that
                // will be handled in state.next() call
                state.unfinalized_entries.insert(idx, txentry);
            }
        }

        let (updated_entries, to_remove) = process_unfinalized_entries(
            &state.unfinalized_entries,
            ops.clone(),
            rpc_client.as_ref(),
        )
        .await
        .map_err(|e| {
            error!(%e, "broadcaster exiting");
            e
        })?;

        for idx in to_remove {
            _ = state.unfinalized_entries.remove(&idx);
        }

        state.next(updated_entries, &ops).await?;
    }
}

async fn get_txid(idx: u64, ops: &BroadcastDbOps) -> BroadcasterResult<Buf32> {
    ops.get_txid_async(idx)
        .await?
        .ok_or(BroadcasterError::Other(format!(
            "No txid entry found for idx {}",
            idx
        )))
}

async fn get_txid_str(idx: u64, ops: &BroadcastDbOps) -> BroadcasterResult<String> {
    let txid: Buf32 = get_txid(idx, ops).await?;
    let mut id = txid.0;
    id.reverse();
    Ok(hex::encode(id))
}

/// Processes unfinalized entries and returns entries idxs that are finalized
async fn process_unfinalized_entries(
    unfinalized_entries: &BTreeMap<u64, L1TxEntry>,
    ops: Arc<BroadcastDbOps>,
    rpc_client: &(impl SeqL1Client + L1Client),
) -> BroadcasterResult<(BTreeMap<u64, L1TxEntry>, Vec<u64>)> {
    let mut to_remove = Vec::new();
    let mut updated_entries = BTreeMap::new();

    for (idx, txentry) in unfinalized_entries.iter() {
        info!(%idx, "processing txentry");
        let updated_status = handle_entry(rpc_client, txentry, *idx, ops.as_ref()).await?;

        if let Some(status) = updated_status {
            let mut new_txentry = txentry.clone();
            new_txentry.status = status.clone();

            // update in db, maybe this should be moved out of this fn to separate concerns??
            ops.update_tx_entry_async(*idx, new_txentry.clone()).await?;

            // Remove if finalized
            if matches!(status, L1TxStatus::Finalized(_))
                || matches!(status, L1TxStatus::Excluded(_))
            {
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
    rpc_client: &(impl SeqL1Client + L1Client),
    txentry: &L1TxEntry,
    idx: u64,
    ops: &BroadcastDbOps,
) -> BroadcasterResult<Option<L1TxStatus>> {
    let txid = get_txid(idx, ops).await?;
    match txentry.status {
        L1TxStatus::Unpublished => {
            // Try to publish
            match send_tx(txentry.tx_raw(), rpc_client).await {
                Ok(_) => {
                    info!(%idx, %txid, "Successfully published tx");
                    Ok(Some(L1TxStatus::Published))
                }
                Err(PublishError::MissingInputsOrSpent) => {
                    warn!(
                        %idx,
                        %txid,
                        "tx excluded while broadcasting due to missing or spent inputs"
                    );
                    Ok(Some(L1TxStatus::Excluded(
                        ExcludeReason::MissingInputsOrSpent,
                    )))
                }
                Err(PublishError::Other(msg)) => {
                    warn!(%idx, %msg, %txid, "tx excluded while broadcasting");
                    Err(BroadcasterError::Other(msg))
                }
            }
        }
        L1TxStatus::Published | L1TxStatus::Confirmed(_) => {
            // check for confirmations
            let txid = Txid::from_slice(txid.0.as_slice())
                .map_err(|e| BroadcasterError::Other(e.to_string()))?;
            let txinfo = rpc_client
                .get_transaction_info(txid)
                .await
                .map_err(|e| BroadcasterError::Other(e.to_string()))?;
            match txinfo.confirmations {
                0 if matches!(txentry.status, L1TxStatus::Confirmed(_h)) => {
                    // If the confirmations of a txn that is already confirmed is 0 then there is
                    // something wrong, possibly a reorg, so just set it to unpublished
                    Ok(Some(L1TxStatus::Unpublished))
                }
                0 => Ok(None),
                c if c >= FINALITY_DEPTH => Ok(Some(L1TxStatus::Finalized(txinfo.block_height()))),
                _ => Ok(Some(L1TxStatus::Confirmed(txinfo.block_height()))),
            }
        }
        L1TxStatus::Finalized(_) => Ok(None), // Nothing to do for finalized tx
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
    client: &(impl SeqL1Client + L1Client),
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
    use alpen_express_rocksdb::test_utils::get_rocksdb_tmp_instance;
    use alpen_test_utils::ArbitraryGenerator;
    use express_storage::ops::l1tx_broadcast::Context;

    use crate::test_utils::TestBitcoinClient;

    use super::*;

    fn get_db() -> Arc<impl TxBroadcastDatabase> {
        let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(BroadcastDb::new(db, dbops));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_ops() -> Arc<BroadcastDbOps> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let ops = Context::new(db).into_ops(pool);
        Arc::new(ops)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let arb = ArbitraryGenerator::new();
        let mut entry: L1TxEntry = arb.generate();
        entry.status = st;
        entry
    }

    #[tokio::test]
    async fn test_handle_unpublished_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Unpublished);

        // Add tx to db
        ops.insert_new_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Published),
            "Status should be if tx is published"
        );
    }

    #[tokio::test]
    async fn test_handle_published_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Published);

        // Add tx to db
        ops.insert_new_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res, None,
            "Status should not change if no confirmations for a published tx"
        );

        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(FINALITY_DEPTH - 1);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed(cl.included_height)),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized(cl.included_height)),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_confirmed_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Confirmed(1));

        // Add tx to db
        ops.insert_new_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Unpublished),
            "Status should revert to unpublished if previously confirmed tx has 0 confirmations"
        );

        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(FINALITY_DEPTH - 1);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed(cl.included_height)),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized(cl.included_height)),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_finalized_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Finalized(1));

        // Add tx to db
        ops.insert_new_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res, None,
            "Status should not change for finalized tx. Should remain the same."
        );

        // This client will return confirmations to be 0
        // NOTE: this should not occur in practice though
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res, None,
            "Status should not change for finalized tx. Should remain the same."
        );
    }

    #[tokio::test]
    async fn test_handle_excluded_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Excluded(ExcludeReason::Other(
            "some reason".to_string(),
        )));

        // Add tx to db
        ops.insert_new_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res, None,
            "Status should not change for excluded tx. Should remain the same."
        );

        // This client will return confirmations to be 0
        // NOTE: this should not occur in practice for a finalized tx though
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res, None,
            "Status should not change for excluded tx. Should remain the same."
        );
    }

    #[tokio::test]
    async fn test_process_unfinalized_entries() {
        let ops = get_ops();
        // Add a couple of txs
        let e1 = gen_entry_with_status(L1TxStatus::Unpublished);
        let i1 = ops
            .insert_new_tx_entry_async([1; 32].into(), e1)
            .await
            .unwrap();
        let e2 = gen_entry_with_status(L1TxStatus::Excluded(ExcludeReason::MissingInputsOrSpent));
        let _i2 = ops
            .insert_new_tx_entry_async([2; 32].into(), e2)
            .await
            .unwrap();

        let e3 = gen_entry_with_status(L1TxStatus::Published);
        let i3 = ops
            .insert_new_tx_entry_async([3; 32].into(), e3)
            .await
            .unwrap();

        let state = BroadcasterState::initialize(&ops).await.unwrap();

        // This client will make the published tx finalized
        let client = TestBitcoinClient::new(FINALITY_DEPTH);
        let cl = Arc::new(client);

        let (new_entries, to_remove) =
            process_unfinalized_entries(&state.unfinalized_entries, ops, cl.as_ref())
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
            L1TxStatus::Finalized(cl.included_height),
            "published tx should be finalized"
        );
    }
}
