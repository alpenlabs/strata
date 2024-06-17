mod builder;

use std::{collections::VecDeque, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

use bitcoin::{consensus::serialize, Address, Transaction};
use tokio::sync::{broadcast::Receiver, mpsc};
use tracing::{info, warn};

use crate::rpc::{types::RawUTXO, BitcoinClient};

use self::builder::{create_inscription_transactions, UtxoParseError, UTXO};

const FINALITY_DEPTH: u64 = 6;
const WORKER_SLEEP_DURATION: u64 = 1; // seconds

enum WriterMsg {}

// TODO: this should be somewhere common to duty executor
#[derive(Clone)]
pub struct L1WriteIntent {
    /// The range of L2 blocks that the intent spans
    pub block_range: (u64, u64),

    /// Proof of the batch execution
    pub proof_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's proof signature
    pub proof_signature: Vec<u8>,

    /// Actual batch data to be posted. Possible state-diff
    pub batch_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's Batch signature
    pub batch_signature: Vec<u8>,
}

// TODO: this comes from config or inside L1WriteIntent
const SEQUENCER_PUBKEY: &[u8] = &[];
// This probably should be in config, or we can just pay dust
const AMOUNT_TO_REVEAL_TXN: u64 = 1000;
const ROLLUP_NAME: &str = "alpen";

pub struct TxnWithStatus {
    txn: Transaction,
    status: BitcoinTxnStatus,
}

impl TxnWithStatus {
    /// Create a new object corresponding a transaction sent to mempool
    pub fn new_mempool_txn(txn: Transaction) -> Self {
        Self {
            txn,
            status: BitcoinTxnStatus::InMempool,
        }
    }

    pub fn txn(&self) -> &Transaction {
        &self.txn
    }

    pub fn status(&self) -> &BitcoinTxnStatus {
        &self.status
    }
}

pub enum BitcoinTxnStatus {
    InMempool,
    Confirmed,
    Finalized,
}

pub async fn writer_control_task(
    mut duty_receiver: Receiver<L1WriteIntent>,
    rpc_client: Arc<BitcoinClient>,
) -> anyhow::Result<()> {
    let (sender, receiver) = mpsc::channel::<WriterMsg>(100);
    // TODO: appropriately get change address, possibly through config
    let change_address = Address::from_str("000")?.require_network(rpc_client.network())?;

    let queue: VecDeque<TxnWithStatus> = initialize_writer()?;

    let queue = Arc::new(Mutex::new(queue));

    tokio::spawn(watch_and_retry_task(queue.clone(), rpc_client.clone()));

    loop {
        let write_intent = duty_receiver.recv().await?;
        let utxos = rpc_client.get_utxos().await?;
        let utxos = utxos
            .into_iter()
            .map(|x| <RawUTXO as TryInto<UTXO>>::try_into(x))
            .into_iter()
            .collect::<Result<Vec<UTXO>, UtxoParseError>>()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let fee_rate = rpc_client.estimate_smart_fee().await?;
        let (commit, reveal) = create_inscription_transactions(
            ROLLUP_NAME,
            write_intent,
            SEQUENCER_PUBKEY.to_vec(),
            utxos,
            change_address.clone(),
            AMOUNT_TO_REVEAL_TXN,
            fee_rate,
            rpc_client.network(),
        )?;

        // Send to bitcoin
        rpc_client
            .send_raw_transaction(hex::encode(serialize(&commit)))
            .await?;

        // If succeeded, the txn is in mempool, add to queue for tracking
        {
            let mut q = queue.lock().await;
            q.push_front(TxnWithStatus::new_mempool_txn(commit));
        }
        rpc_client
            .send_raw_transaction(hex::encode(serialize(&reveal)))
            .await?;
        // If succeeded, the txn is in mempool, add to queue for tracking
        {
            let mut q = queue.lock().await;
            q.push_front(TxnWithStatus::new_mempool_txn(reveal));
        }
    }
}

pub fn initialize_writer() -> anyhow::Result<VecDeque<TxnWithStatus>> {
    // TODO: possibly load queue from persistent store.
    Ok(Default::default())
}

/// Watches for inscription transactions status in bitcoin and retries until they are confirmed
pub async fn watch_and_retry_task(
    q: Arc<Mutex<VecDeque<TxnWithStatus>>>,
    rpc_client: Arc<BitcoinClient>,
) {
    // TODO: Handle reorg
    loop {
        let txn = {
            let mut q = q.lock().await;
            q.pop_back()
        };
        match txn {
            Some(mut txn) => {
                let confs_res = rpc_client
                    .get_transaction_confirmations(txn.txn().compute_txid().to_string())
                    .await;
                match confs_res {
                    Ok(confs) => {
                        if confs > FINALITY_DEPTH {
                            info!(txn = %txn.txn().compute_txid(), "Transaction finalized");
                            txn.status = BitcoinTxnStatus::Finalized;
                            // No need to push it to the queue again
                            continue;
                        } else if confs > 0 {
                            txn.status = BitcoinTxnStatus::Confirmed;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Error fetching txn confirmations");
                        // Resend
                        // TODO: possibly resend with higher fees
                        let res = rpc_client
                            .send_raw_transaction(hex::encode(serialize(txn.txn())))
                            .await;
                        match res {
                            Ok(_) => {}
                            Err(e) => {
                                warn!(error = %e, "Couldn't resend transaction");
                                // The txn will be enqueued again below, but should we just discard
                                // it? And just continue here.
                            }
                        }
                    }
                }
                // Push the txn back to the queue
                {
                    let mut q = q.lock().await;
                    q.push_front(txn)
                }
            }
            None => {}
        }
        let _ = tokio::time::sleep(Duration::from_secs(WORKER_SLEEP_DURATION)).await;
    }
}
