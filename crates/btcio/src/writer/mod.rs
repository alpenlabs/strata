mod builder;

use std::{collections::VecDeque, str::FromStr, sync::Arc, time::Duration};

#[cfg(test)]
use arbitrary::Arbitrary;
use bitcoin::{consensus::serialize, Address, Transaction};
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tracing::{info, warn};

use alpen_vertex_db::traits::L1DataProvider;
use alpen_vertex_primitives::buf::Buf32;

use self::builder::{create_inscription_transactions, UtxoParseError, UTXO};
use crate::rpc::{types::RawUTXO, BitcoinClient};

// TODO: this comes from config or inside L1WriteIntent
const SEQUENCER_PUBKEY: &[u8] = &[];
// This probably should be in config, or we can just pay dust
const AMOUNT_TO_REVEAL_TXN: u64 = 1000;
const ROLLUP_NAME: &str = "alpen";

const FINALITY_DEPTH: u64 = 6;
const WORKER_SLEEP_DURATION: u64 = 1; // seconds
const SEQUENCER_CHANGE_ADDRESS: &str = "00000000000"; // TODO: change this

// TODO: this should be somewhere common to duty executor
#[derive(Clone)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct L1WriteIntent {
    /// The range of L2 blocks that the intent spans
    pub l2_block_range: (u64, u64),

    /// The range of L1 blocks that the intent spans
    pub l1_block_range: (u64, u64),

    /// The block hashes corresponding to the l1 block height range
    pub l1_block_hash_range: (Buf32, Buf32),

    /// Proof of the batch execution
    pub proof_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's proof signature
    pub proof_signature: Vec<u8>,

    /// Actual batch data to be posted. Possible state-diff
    pub batch_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's Batch signature
    pub batch_signature: Vec<u8>,
}

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

pub async fn writer_control_task<D>(
    mut duty_receiver: Receiver<L1WriteIntent>,
    rpc_client: Arc<BitcoinClient>,
    db: Arc<D>,
) -> anyhow::Result<()>
where
    D: L1DataProvider,
{
    let queue: VecDeque<TxnWithStatus> = initialize_writer()?;

    let queue = Arc::new(Mutex::new(queue));

    tokio::spawn(watch_and_retry_task(queue.clone(), rpc_client.clone()));

    loop {
        let write_intent = duty_receiver.recv().await?;
        let (commit, reveal) = create_inscriptions_from_intent(&write_intent, &rpc_client).await?;

        // Send to bitcoin only after checking if the write_intent's block range is finalized
        // And then add to the queue for tracking. Each items in the queue will be checked if
        // present in L1 or not until they are finalized. If not present in mempool or in chain,
        // keep resending the transaction.
        if send_if_finalized(&write_intent, &commit, &reveal, &rpc_client, &db).await? {
            let mut q = queue.lock().await;
            q.push_front(TxnWithStatus::new_mempool_txn(commit));
            q.push_front(TxnWithStatus::new_mempool_txn(reveal));
        }
    }
}

async fn create_inscriptions_from_intent(
    write_intent: &L1WriteIntent,
    rpc_client: &BitcoinClient,
) -> anyhow::Result<(Transaction, Transaction)> {
    let change_address =
        Address::from_str(SEQUENCER_CHANGE_ADDRESS)?.require_network(rpc_client.network())?;
    let utxos = rpc_client.get_utxos().await?;
    let utxos = utxos
        .into_iter()
        .map(|x| <RawUTXO as TryInto<UTXO>>::try_into(x))
        .into_iter()
        .collect::<Result<Vec<UTXO>, UtxoParseError>>()
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    let fee_rate = rpc_client.estimate_smart_fee().await?;
    create_inscription_transactions(
        ROLLUP_NAME,
        &write_intent,
        SEQUENCER_PUBKEY.to_vec(),
        utxos,
        change_address.clone(),
        AMOUNT_TO_REVEAL_TXN,
        fee_rate,
        rpc_client.network(),
    )
}

async fn send_if_finalized<D>(
    intent: &L1WriteIntent,
    commit: &Transaction,
    reveal: &Transaction,
    rpc_client: &Arc<BitcoinClient>,
    db: &Arc<D>,
) -> anyhow::Result<bool>
where
    D: L1DataProvider,
{
    let block_height = intent.l1_block_range.1;
    let block_hash = intent.l1_block_hash_range.1;
    let mf = db.get_block_manifest(block_height)?;
    if let Some(mf) = mf {
        if mf.block_hash() != block_hash {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }
    rpc_client
        .send_raw_transaction(hex::encode(serialize(&commit)))
        .await?;
    rpc_client
        .send_raw_transaction(hex::encode(serialize(&reveal)))
        .await?;
    Ok(true)
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
    // NOTE: No need to handle reorg as inscriptions are sent only after the bitcoin block range
    // have been finalized
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
                    Ok(confs) if confs > FINALITY_DEPTH => {
                        info!(txn = %txn.txn().compute_txid(), "Transaction finalized");
                        txn.status = BitcoinTxnStatus::Finalized;
                        // No need to push it to the queue again
                        continue;
                    }
                    Ok(confs) if confs > 0 => {
                        txn.status = BitcoinTxnStatus::Confirmed;
                        // No need to resend, but need to push in the queue, which happens
                        // at the end of the loop
                    }
                    Ok(_) => {}
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
                                // it? And thus 'continue' here.
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
