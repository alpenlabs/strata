use std::{collections::VecDeque, sync::Arc, time::Duration};

use alpen_vertex_primitives::{
    buf::Buf32,
    l1::{BitcoinTxnStatus, TxnWithStatus},
};
use bitcoin::Transaction;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracing::*;

use alpen_vertex_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    DbResult,
};

use super::{
    builder::{create_inscription_transactions, sign_blob_with_private_key, UtxoParseError, UTXO},
    config::{InscriptionFeePolicy, WriterConfig},
    state::WriterState,
};
use crate::rpc::{types::RawUTXO, BitcoinClient};

const FINALITY_DEPTH: u64 = 6;

pub async fn writer_control_task<D>(
    intent_rx: Receiver<Vec<u8>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()>
where
    D: SequencerDatabase + Sync + Send + 'static,
{
    info!("Starting writer control task");

    let state = initialize_writer_state(db.clone())?;
    let state = Arc::new(Mutex::new(state));
    let st_clone = state.clone();

    tokio::spawn(transactions_tracker_task(
        st_clone,
        rpc_client.clone(),
        config.clone(),
    ));

    let _ = listen_for_write_intents(intent_rx, rpc_client, config, state, db).await;

    Ok(())
}

async fn listen_for_write_intents<D>(
    mut intent_rx: Receiver<Vec<u8>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
    state: Arc<Mutex<WriterState<D>>>,
    db: Arc<D>,
) -> anyhow::Result<()>
where
    D: SequencerDatabase + Sync + Send + 'static,
{
    loop {
        let write_intent = intent_rx
            .recv()
            .await
            .ok_or(anyhow::anyhow!("Intent channel closed"))?;

        if let Err(e) = handle_intent(
            write_intent,
            db.clone(),
            &rpc_client,
            &config,
            state.clone(),
        )
        .await
        {
            error!(%e, "Failed to handle intent");
        }
    }
}

async fn handle_intent<D: SequencerDatabase>(
    intent: Vec<u8>,
    db: Arc<D>,
    rpc_client: &Arc<BitcoinClient>,
    config: &WriterConfig,
    state: Arc<Mutex<WriterState<D>>>,
) -> anyhow::Result<()> {
    // If it is already present in the db and corresponding txns are created, return

    let hash: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(&intent);
        hasher.finalize().into()
    };

    let blobid = Buf32(hash.into());
    let seqprov = db.sequencer_provider();

    match seqprov.get_blob_by_id(blobid.clone())? {
        Some(_) => {
            warn!("duplicate write intent {hash:?}. Checking if L1 transaction exits");
            if seqprov.get_txidx_for_blob(blobid)?.is_some() {
                warn!("L1 txn exists, ignoring the intent");
                return Ok(());
            }
        }
        None => {
            // Store in db
            let _ = db.sequencer_store().put_blob(blobid, intent.clone())?;
        }
    }

    // Create commit reveal txns and atomically store in db as well
    let (commit, reveal) = create_inscriptions_from_intent(&intent, &rpc_client, &config).await?;

    let commit_tx = TxnWithStatus::new_unsent(commit);
    let reveal_tx = TxnWithStatus::new_unsent(reveal);

    let _reveal_txidx =
        db.sequencer_store()
            .put_commit_reveal_txns(blobid, commit_tx.clone(), reveal_tx.clone());

    // Update the writer state by adding the txns which will be used by tracker
    state.lock().await.add_new_txn(commit_tx);
    state.lock().await.add_new_txn(reveal_tx);

    Ok(())
}

async fn create_inscriptions_from_intent(
    write_intent: &[u8],
    rpc_client: &BitcoinClient,
    config: &WriterConfig,
) -> anyhow::Result<(Transaction, Transaction)> {
    let (signature, pub_key) = sign_blob_with_private_key(&write_intent, &config.private_key)?;
    let utxos = rpc_client.get_utxos().await?;
    let utxos = utxos
        .into_iter()
        .map(|x| <RawUTXO as TryInto<UTXO>>::try_into(x))
        .into_iter()
        .collect::<Result<Vec<UTXO>, UtxoParseError>>()
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    let fee_rate = match config.inscription_fee_policy {
        InscriptionFeePolicy::Smart => rpc_client.estimate_smart_fee().await?,
        InscriptionFeePolicy::Fixed(val) => val,
    };
    create_inscription_transactions(
        &config.rollup_name,
        &write_intent,
        signature,
        pub_key,
        utxos,
        config.change_address.clone(),
        config.amount_for_reveal_txn,
        fee_rate,
        rpc_client.network(),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn initialize_writer_state<D: SequencerDatabase>(db: Arc<D>) -> anyhow::Result<WriterState<D>> {
    // The idea here is to get the latest blob, corresponding l1 txidx, and loop backwards until we
    // reach upto the finalized txns while we are collecting the visited txns in a queue.

    if let Some(last_idx) = db.sequencer_provider().get_last_blob_idx()? {
        let blobid = db.sequencer_provider().get_blobid_for_blob_idx(last_idx)?;

        // NOTE: This is the reveal txidx
        if let Some(txidx) = db.sequencer_provider().get_txidx_for_blob(blobid)? {
            let queue = create_txn_queue(txidx, db.clone())?;
            return Ok(WriterState::new(db, queue));
        }
    }
    return Ok(WriterState::new_empty(db));
}

fn create_txn_queue<D: SequencerDatabase>(
    txidx: u64,
    db: Arc<D>,
) -> DbResult<VecDeque<TxnWithStatus>> {
    let seqprov = db.sequencer_provider();
    let mut queue = VecDeque::default();
    let mut txidx = txidx;
    loop {
        if txidx <= 0 {
            break;
        }
        // fetch commit and reveal txns
        let reveal_txn = seqprov
            .get_l1_txn(txidx)?
            .expect("Inconsistent existence of transactions");
        let commit_txn = seqprov
            .get_l1_txn(txidx - 1)?
            .expect("Inconsistsent exisence of transactions");

        if *reveal_txn.status() == BitcoinTxnStatus::Finalized {
            break;
        } else {
            queue.push_front(reveal_txn);
            queue.push_front(commit_txn);
        }
        txidx -= 2;
    }
    return Ok(queue);
}

/// Watches for inscription transactions status in bitcoin and resends if not in mempool, until they are confirmed
async fn transactions_tracker_task<D: SequencerDatabase>(
    state: Arc<Mutex<WriterState<D>>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
) -> anyhow::Result<()> {
    // TODO: better interval values and placement in the loop

    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    loop {
        interval.as_mut().tick().await;

        let txns: VecDeque<_> = {
            let st = state.lock().await;
            st.txns_queue.iter().cloned().collect()
        };

        for (idx, txn) in txns.iter().enumerate() {
            let mut status = BitcoinTxnStatus::Unsent;

            match txn.status {
                BitcoinTxnStatus::Unsent => {
                    // NOTE: when sending errors, set it's status to unsent so that it is tried
                    // again later
                    if let Ok(_) = rpc_client.send_raw_transaction(txn.txn_raw.clone()).await {
                        status = BitcoinTxnStatus::InMempool;
                    } else {
                        status = BitcoinTxnStatus::Unsent;
                    }
                }

                BitcoinTxnStatus::InMempool | BitcoinTxnStatus::Confirmed => {
                    status = check_confirmations_and_resend_txn(&txn, &rpc_client).await;
                }

                BitcoinTxnStatus::Finalized => {
                    state.lock().await.finalize_txn(idx)?;
                    continue;
                }
            }
            // Update the txn
            {
                state.lock().await.update_txn(idx, status)?;
            }
        }
    }
}

async fn check_confirmations_and_resend_txn(
    txn: &TxnWithStatus,
    rpc_client: &Arc<BitcoinClient>,
) -> BitcoinTxnStatus {
    let confs = rpc_client.get_transaction_confirmations(txn.txid().0).await;

    match confs {
        Ok(confs) if confs > FINALITY_DEPTH => {
            info!(txn = %hex::encode(txn.txid().0), "Transaction finalized");
            BitcoinTxnStatus::Finalized
        }

        Ok(confs) if confs > 0 => BitcoinTxnStatus::Confirmed,

        Ok(_) => BitcoinTxnStatus::InMempool,

        Err(e) => {
            warn!(error = %e, "Error fetching txn confirmations");

            // TODO: possibly resend with higher fees
            let _res = rpc_client
                .send_raw_transaction(hex::encode(txn.txn_raw()))
                .await
                .map_err(|e| {
                    warn!(error = %e, "Couldn't resend transaction");
                    e
                });
            BitcoinTxnStatus::Unsent
        }
    }
}

// TODO: write unit tests
