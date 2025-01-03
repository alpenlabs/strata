use std::{collections::BTreeMap, sync::Arc, time::Duration};

use bitcoin::{hashes::Hash, Txid};
use strata_db::types::{L1TxEntry, L1TxStatus};
use strata_primitives::params::Params;
use strata_storage::{ops::l1tx_broadcast, BroadcastDbOps};
use tokio::sync::mpsc::Receiver;
use tracing::*;

use crate::{
    broadcaster::{
        error::{BroadcasterError, BroadcasterResult},
        state::BroadcasterState,
    },
    rpc::traits::{Broadcaster, Wallet},
};

const BROADCAST_POLL_INTERVAL: u64 = 1_000; // millis

/// Broadcasts the next blob to be sent
pub async fn broadcaster_task(
    rpc_client: Arc<impl Broadcaster + Wallet>,
    ops: Arc<l1tx_broadcast::BroadcastDbOps>,
    mut entry_receiver: Receiver<(u64, L1TxEntry)>,
    params: Arc<Params>,
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
                let txid: Option<Txid> = ops.get_txid_async(idx).await?.map(Into::into);
                info!(%idx, ?txid, "Received txentry");

                // Insert into state's unfinalized entries. Need not update next_idx because that
                // will be handled in state.next() call
                state.unfinalized_entries.insert(idx, txentry);
            }
        }

        let (updated_entries, to_remove) = process_unfinalized_entries(
            &state.unfinalized_entries,
            ops.clone(),
            rpc_client.as_ref(),
            params.as_ref(),
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

/// Processes unfinalized entries and returns entries idxs that are finalized
async fn process_unfinalized_entries(
    unfinalized_entries: &BTreeMap<u64, L1TxEntry>,
    ops: Arc<BroadcastDbOps>,
    rpc_client: &(impl Broadcaster + Wallet),
    params: &Params,
) -> BroadcasterResult<(BTreeMap<u64, L1TxEntry>, Vec<u64>)> {
    let mut to_remove = Vec::new();
    let mut updated_entries = BTreeMap::new();

    for (idx, txentry) in unfinalized_entries.iter() {
        debug!(?txentry.status, %idx, "processing txentry");
        let updated_status = handle_entry(rpc_client, txentry, *idx, ops.as_ref(), params).await?;
        debug!(?updated_status, %idx, "updated status handled");

        if let Some(status) = updated_status {
            let mut new_txentry = txentry.clone();
            new_txentry.status = status.clone();

            // update in db, maybe this should be moved out of this fn to separate concerns??
            ops.put_tx_entry_by_idx_async(*idx, new_txentry.clone())
                .await?;

            // Remove if finalized or has invalid inputs
            if matches!(status, L1TxStatus::Finalized { confirmations: _ })
                || matches!(status, L1TxStatus::InvalidInputs)
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
    rpc_client: &(impl Broadcaster + Wallet),
    txentry: &L1TxEntry,
    idx: u64,
    ops: &BroadcastDbOps,
    params: &Params,
) -> BroadcasterResult<Option<L1TxStatus>> {
    let txid = ops
        .get_txid_async(idx)
        .await?
        .ok_or(BroadcasterError::TxNotFound(idx))?;
    match txentry.status {
        L1TxStatus::Unpublished => {
            // Try to publish
            let tx = txentry.try_to_tx().expect("could not deserialize tx");
            trace!(%idx, ?tx, "Publishing tx");
            match rpc_client.send_raw_transaction(&tx).await {
                Ok(_) => {
                    info!(%idx, %txid, "Successfully published tx");
                    Ok(Some(L1TxStatus::Published))
                }
                Err(err) if err.is_missing_or_invalid_input() => {
                    warn!(?err, %idx, %txid, "tx excluded due to invalid inputs");

                    Ok(Some(L1TxStatus::InvalidInputs))
                }
                Err(err) => {
                    warn!(%idx, ?err, %txid, "errored while broadcasting");
                    Err(BroadcasterError::Other(err.to_string()))
                }
            }
        }
        L1TxStatus::Published | L1TxStatus::Confirmed { confirmations: _ } => {
            // Check for confirmations
            let txid = Txid::from_slice(txid.0.as_slice())
                .map_err(|e| BroadcasterError::Other(e.to_string()))?;
            let txinfo_res = rpc_client.get_transaction(&txid).await;

            debug!(?txentry.status, ?txinfo_res, ?txid, "check get transaction");
            let new_status = match txinfo_res {
                Ok(info) => {
                    if info.confirmations == 0 && txentry.status == L1TxStatus::Published {
                        L1TxStatus::Published
                    } else if info.confirmations == 0 {
                        // If it was confirmed before and now it is 0, L1 reorged.
                        // So, set it to Unpublished
                        L1TxStatus::Unpublished
                    } else if info.confirmations >= params.rollup().l1_reorg_safe_depth.into() {
                        L1TxStatus::Finalized {
                            confirmations: info.confirmations,
                        }
                    } else {
                        L1TxStatus::Confirmed {
                            confirmations: info.confirmations,
                        }
                    }
                }
                Err(e) => {
                    // If for some reasons tx is not found even if it was already
                    // published/confirmed, set it to unpublished.
                    if e.is_tx_not_found() {
                        L1TxStatus::Unpublished
                    } else {
                        return Err(BroadcasterError::Other(e.to_string()));
                    }
                }
            };
            Ok(Some(new_status))
        }
        L1TxStatus::Finalized { confirmations: _ } => Ok(None),
        L1TxStatus::InvalidInputs => Ok(None),
    }
}

#[cfg(test)]
mod test {
    use bitcoin::{consensus, Transaction};
    use strata_db::traits::BroadcastDatabase;
    use strata_rocksdb::{
        broadcaster::db::{BroadcastDb, L1BroadcastDb},
        test_utils::get_rocksdb_tmp_instance,
    };
    use strata_storage::ops::l1tx_broadcast::Context;
    use strata_test_utils::l2::gen_params;

    use super::*;
    use crate::test_utils::{TestBitcoinClient, SOME_TX};

    fn get_db() -> Arc<impl BroadcastDatabase> {
        let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(L1BroadcastDb::new(db, dbops));
        Arc::new(BroadcastDb::new(bcastdb))
    }

    fn get_ops() -> Arc<BroadcastDbOps> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let ops = Context::new(db.l1_broadcast_db().clone()).into_ops(pool);
        Arc::new(ops)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let tx: Transaction = consensus::encode::deserialize_hex(SOME_TX).unwrap();
        let mut entry = L1TxEntry::from_tx(&tx);
        entry.status = st;
        entry
    }

    fn get_params() -> Arc<Params> {
        Arc::new(gen_params())
    }

    #[tokio::test]
    async fn test_handle_unpublished_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Unpublished);

        // Add tx to db
        ops.put_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), get_params().as_ref())
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
        ops.put_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);
        let params = get_params();

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Published),
            "Status should not change if no confirmations for a published tx"
        );

        let reorg_depth: u64 = params.rollup().l1_reorg_safe_depth.into();
        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(reorg_depth - 1);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed {
                confirmations: cl.confs
            }),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(reorg_depth);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized {
                confirmations: cl.confs
            }),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_confirmed_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Confirmed { confirmations: 1 });

        // Add tx to db
        ops.put_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        // This client will return confirmations to be 0
        let client = TestBitcoinClient::new(0);
        let cl = Arc::new(client);

        let params = get_params();
        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Unpublished),
            "Status should revert to reorged if previously confirmed tx has 0 confirmations"
        );

        let reorg_depth = params.rollup().l1_reorg_safe_depth as u64;
        // This client will return confirmations to be finality_depth - 1
        let client = TestBitcoinClient::new(reorg_depth - 1);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Confirmed {
                confirmations: cl.confs
            }),
            "Status should be confirmed if 0 < confirmations < finality_depth"
        );

        // This client will return confirmations to be finality_depth
        let client = TestBitcoinClient::new(reorg_depth);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
            .await
            .unwrap();
        assert_eq!(
            res,
            Some(L1TxStatus::Finalized {
                confirmations: cl.confs
            }),
            "Status should be confirmed if confirmations >= finality_depth"
        );
    }

    #[tokio::test]
    async fn test_handle_finalized_entry() {
        let ops = get_ops();
        let e = gen_entry_with_status(L1TxStatus::Finalized { confirmations: 1 });

        // Add tx to db
        ops.put_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        let params = get_params();
        let reorg_depth = params.rollup().l1_reorg_safe_depth as u64;
        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(reorg_depth);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
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

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
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
        let e = gen_entry_with_status(L1TxStatus::InvalidInputs);

        // Add tx to db
        ops.put_tx_entry_async([1; 32].into(), e.clone())
            .await
            .unwrap();

        let params = get_params();
        let reorg_depth = params.rollup().l1_reorg_safe_depth as u64;
        // This client will return confirmations to be Finality depth
        let client = TestBitcoinClient::new(reorg_depth);
        let cl = Arc::new(client);

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
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

        let res = handle_entry(cl.as_ref(), &e, 0, ops.as_ref(), params.as_ref())
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
        let i1 = ops.put_tx_entry_async([1; 32].into(), e1).await.unwrap();
        let e2 = gen_entry_with_status(L1TxStatus::InvalidInputs);
        let _i2 = ops.put_tx_entry_async([2; 32].into(), e2).await.unwrap();

        let e3 = gen_entry_with_status(L1TxStatus::Published);
        let i3 = ops.put_tx_entry_async([3; 32].into(), e3).await.unwrap();

        let state = BroadcasterState::initialize(&ops).await.unwrap();

        let params = get_params();
        let reorg_depth = params.rollup().l1_reorg_safe_depth as u64;
        // This client will make the published tx finalized
        let client = TestBitcoinClient::new(reorg_depth);
        let cl = Arc::new(client);

        let (new_entries, to_remove) = process_unfinalized_entries(
            &state.unfinalized_entries,
            ops,
            cl.as_ref(),
            params.as_ref(),
        )
        .await
        .unwrap();

        // The published tx which got finalized should be removed
        assert_eq!(
            to_remove,
            vec![i3.unwrap()],
            "Finalized tx should be in to_remove list"
        );

        assert_eq!(
            new_entries.get(&i1.unwrap()).unwrap().status,
            L1TxStatus::Published,
            "unpublished tx should be published"
        );
        assert_eq!(
            new_entries.get(&i3.unwrap()).unwrap().status,
            L1TxStatus::Finalized {
                confirmations: cl.confs
            },
            "published tx should be finalized"
        );
    }
}
