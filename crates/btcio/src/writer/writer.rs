use std::{collections::VecDeque, sync::Arc, time::Duration};

use alpen_vertex_primitives::{
    buf::Buf32,
    l1::{BitcoinTxnStatus, TxnWithStatus},
};
use bitcoin::{consensus::serialize, Transaction};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracing::*;

use alpen_vertex_db::{
    traits::{Database, SeqDataProvider, SeqDataStore},
    DbResult,
};

use super::{
    builder::{create_inscription_transactions, sign_blob_with_private_key, UtxoParseError, UTXO},
    config::{InscriptionFeePolicy, WriterConfig},
};
use crate::rpc::{types::RawUTXO, BitcoinClient};

// This probably should be in config, or we can just pay dust
const AMOUNT_TO_REVEAL_TXN: u64 = 1000;

const FINALITY_DEPTH: u64 = 6;

#[derive(Default)]
struct WriterState<D> {
    /// The queue of transactions that need to be sent to L1 or whose status needs to be tracked
    txns_queue: VecDeque<TxnWithStatus>,

    /// The idx of the first transaction. This is set while the writer control_task is initialized
    first_txn_idx: u64,

    /// database to access the L1 transactions
    db: Arc<D>,
}

impl<D: Database> WriterState<D> {
    pub fn new(db: Arc<D>, txns_queue: VecDeque<TxnWithStatus>, first_txn_idx: u64) -> Self {
        Self {
            db,
            txns_queue,
            first_txn_idx,
        }
    }

    pub fn new_empty(db: Arc<D>) -> Self {
        Self::new(db, Default::default(), Default::default())
    }

    pub fn add_new_txn(&mut self, txn: TxnWithStatus) {
        self.txns_queue.push_back(txn)
    }

    pub fn finalize_txn(&mut self, idx: usize) -> DbResult<()> {
        todo!()
    }

    pub fn update_txn(&mut self, idx: usize, status: BitcoinTxnStatus) -> DbResult<()> {
        //
        todo!()
    }
}

pub async fn writer_control_task<D>(
    mut intent_rx: Receiver<Vec<u8>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()>
where
    D: Database + Sync + Send + 'static,
{
    info!("Starting writer control task");
    let state = Arc::new(Mutex::new(initialize_writer_state(db.clone())?));
    let st_clone = state.clone();

    tokio::spawn(transactions_tracker_task(
        st_clone,
        rpc_client.clone(),
        config.clone(),
    ));

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

async fn handle_intent<D: Database>(
    intent: Vec<u8>,
    db: Arc<D>,
    rpc_client: &Arc<BitcoinClient>,
    config: &WriterConfig,
    state: Arc<Mutex<WriterState<D>>>,
) -> anyhow::Result<()> {
    // Check if it is already present, if so return
    let hash: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(&intent);
        hasher.finalize().into()
    };
    let blobid = Buf32(hash.into());
    if db
        .sequencer_provider()
        .get_blob_by_id(blobid.clone())?
        .is_some()
    {
        warn!("duplicate write intent {hash:?}");
        return Ok(());
    }

    // Store in db
    let blobidx = db.sequencer_store().put_blob(blobid, intent.clone())?;

    // Create commit reveal txns and store in db as well
    let (commit, reveal) = create_inscriptions_from_intent(&intent, &rpc_client, &config).await?;

    let commit_tx = TxnWithStatus::new_unsent(commit);
    let reveal_tx = TxnWithStatus::new_unsent(reveal);

    let reveal_txidx =
        db.sequencer_store()
            .put_commit_reveal_txns(blobidx, commit_tx.clone(), reveal_tx.clone());

    // TODO: associate reveal txidx with blob

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
        AMOUNT_TO_REVEAL_TXN,
        fee_rate,
        rpc_client.network(),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn initialize_writer_state<D: Database>(db: Arc<D>) -> anyhow::Result<WriterState<D>> {
    // The idea here is to get the latest blob, corresponding l1 txidx, and loop backwards until we
    // have the finalized txns while we are collecting the visited txns in a queue.
    let seqprov = db.sequencer_provider();
    let last_idx = seqprov.get_last_blob_idx()?;
    let mut txidx = seqprov.get_txidx_for_blob(last_idx)?; // NOTE: this is the reveal txidx
    let mut queue = VecDeque::default();
    loop {
        if txidx <= 0 {
            break;
        }
        // fetch commit and reveal txns
        let reveal_txn = seqprov.get_l1_txn(txidx)?;
        let commit_txn = seqprov.get_l1_txn(txidx - 1)?;

        if *reveal_txn.status() == BitcoinTxnStatus::Finalized {
            break;
        } else {
            queue.push_front(reveal_txn);
            queue.push_front(commit_txn);
        }
        txidx -= 2;
    }

    let first_txidx = txidx + 1; // txidx is the idx for finalized reveal txn, we want the next non
                                 // finalized commit txns
    Ok(WriterState::new(db, queue, first_txidx))
}

/// Watches for inscription transactions status in bitcoin and resends if not in mempool, until they are confirmed
async fn transactions_tracker_task<D: Database>(
    state: Arc<Mutex<WriterState<D>>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
) -> anyhow::Result<()> {
    // TODO: better interval values and placement in the loop
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    loop {
        interval.as_mut().tick().await;

        let txns = {
            let st = state.lock().await;
            st.txns_queue.clone()
        };
        for (idx, txn) in txns.iter().enumerate() {
            let mut status = BitcoinTxnStatus::Unsent;
            match txn.status {
                BitcoinTxnStatus::Unsent => {
                    // FIXME: when sending errors, set it's status to unsent so that it is tried
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
            // Resend
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
