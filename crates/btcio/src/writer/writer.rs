use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use bitcoin::{consensus::serialize, Transaction};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tracing::*;

use alpen_vertex_db::traits::L1DataProvider;

use super::{
    builder::{create_inscription_transactions, sign_blob_with_private_key, UtxoParseError, UTXO},
    config::{InscriptionFeePolicy, WriterConfig},
};
use crate::rpc::{types::RawUTXO, BitcoinClient};

// This probably should be in config, or we can just pay dust
const AMOUNT_TO_REVEAL_TXN: u64 = 1000;

const FINALITY_DEPTH: u64 = 6;

pub type L1WriteIntent = Vec<u8>;

pub struct TxnWithStatus {
    txn: Transaction,
    status: BitcoinTxnStatus,
}

impl TxnWithStatus {
    /// Create a new object corresponding a transaction sent to mempool
    pub fn new(txn: Transaction, status: BitcoinTxnStatus) -> Self {
        Self { txn, status }
    }

    /// Create a new object corresponding a transaction sent to mempool
    pub fn new_mempool_txn(txn: Transaction) -> Self {
        Self::new(txn, BitcoinTxnStatus::InMempool)
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
    mut intent_rx: Receiver<L1WriteIntent>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
) -> anyhow::Result<()>
where
    D: L1DataProvider,
{
    let mut cache: HashSet<[u8; 32]> = HashSet::new();

    let queue: VecDeque<TxnWithStatus> = initialize_writer()?;

    let queue = Arc::new(Mutex::new(queue));

    tokio::spawn(watch_and_retry_task(
        queue.clone(),
        rpc_client.clone(),
        config.clone(),
    ));

    loop {
        let write_intent = intent_rx.recv().await?;

        let hash: [u8; 32] = {
            let mut hasher = Sha256::new();
            hasher.update(&write_intent);
            hasher.finalize().into()
        };
        if cache.contains(&hash) {
            debug!("duplicate write intent {hash:?}");
            continue;
        }
        let (commit, reveal) =
            create_inscriptions_from_intent(&write_intent, &rpc_client, &config).await?;

        send_commit_reveal(&commit, &reveal, &rpc_client).await?;
        {
            let mut q = queue.lock().await;
            q.push_front(TxnWithStatus::new_mempool_txn(commit));
            q.push_front(TxnWithStatus::new_mempool_txn(reveal));
        }
        cache.insert(hash);
    }
}

async fn create_inscriptions_from_intent(
    write_intent: &L1WriteIntent,
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

async fn send_commit_reveal(
    commit: &Transaction,
    reveal: &Transaction,
    rpc_client: &Arc<BitcoinClient>,
) -> anyhow::Result<()> {
    rpc_client.send_raw_transaction(serialize(&commit)).await?;
    rpc_client.send_raw_transaction(serialize(&reveal)).await?;
    Ok(())
}

pub fn initialize_writer() -> anyhow::Result<VecDeque<TxnWithStatus>> {
    // TODO: possibly load queue from persistent store.
    Ok(Default::default())
}

/// Watches for inscription transactions status in bitcoin and retries until they are confirmed
pub async fn watch_and_retry_task(
    q: Arc<Mutex<VecDeque<TxnWithStatus>>>,
    rpc_client: Arc<BitcoinClient>,
    config: WriterConfig,
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
                        let _res = rpc_client
                            .send_raw_transaction(hex::encode(serialize(txn.txn())))
                            .await
                            .map_err(|e| {
                                warn!(error = %e, "Couldn't resend transaction");
                                e
                            });
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
        let _ = tokio::time::sleep(Duration::from_millis(config.poll_duration_ms)).await;
    }
}
