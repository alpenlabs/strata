use std::{collections::VecDeque, sync::Arc, time::Duration};

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::da_blob::{BlobDest, BlobIntent};
use bitcoin::Transaction;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracing::*;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{L1TxnStatus, TxnStatusEntry},
    DbResult,
};

use super::{
    builder::{create_inscription_transactions, UtxoParseError, UTXO},
    config::{InscriptionFeePolicy, WriterConfig},
    state::WriterState,
};
use crate::rpc::{
    traits::{L1Client, SeqL1Client},
    types::RawUTXO,
};

const FINALITY_DEPTH: u64 = 6;

pub async fn writer_control_task<D: SequencerDatabase + Send + Sync + 'static>(
    intent_rx: Receiver<BlobIntent>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
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
    mut intent_rx: Receiver<BlobIntent>,
    rpc_client: Arc<impl SeqL1Client>,
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

        // Ignore the intents not meant for L1
        if write_intent.dest() != BlobDest::L1 {
            continue;
        }

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
    intent: BlobIntent,
    db: Arc<D>,
    rpc_client: &Arc<impl SeqL1Client>,
    config: &WriterConfig,
    state: Arc<Mutex<WriterState<D>>>,
) -> anyhow::Result<()> {
    // If it is already present in the db and corresponding txns are created, return

    let blobid = calculate_intent_hash(&intent.payload()); // TODO: should we use the commitment
                                                           // here?
    let seqprov = db.sequencer_provider();

    match seqprov.get_blob_by_id(blobid.clone())? {
        Some(_) => {
            warn!("duplicate write intent {blobid:?}. Checking if L1 transaction exists");
            if seqprov.get_reveal_txidx_for_blob(blobid)?.is_some() {
                warn!("L1 txn exists, ignoring the intent");
                return Ok(());
            }
        }
        None => {
            // Store in db
            let _ = db
                .sequencer_store()
                .put_blob(blobid, intent.payload().to_vec())?;
        }
    }

    // Create commit reveal txns and atomically store in db as well
    let (commit, reveal) = create_inscriptions_from_intent(&intent, &rpc_client, &config).await?;

    let commit_tx = TxnStatusEntry::from_txn_unsent(&commit);
    let reveal_tx = TxnStatusEntry::from_txn_unsent(&reveal);

    let _reveal_txidx =
        db.sequencer_store()
            .put_commit_reveal_txns(blobid, commit_tx.clone(), reveal_tx.clone());

    // Update the writer state by adding the txns which will be used by tracker
    state.lock().await.add_new_txn(commit_tx);
    state.lock().await.add_new_txn(reveal_tx);

    Ok(())
}

fn calculate_intent_hash(intent: &[u8]) -> Buf32 {
    let hash: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(intent);
        hasher.finalize().into()
    };
    Buf32(hash.into())
}

async fn create_inscriptions_from_intent(
    write_intent: &BlobIntent,
    rpc_client: &Arc<impl SeqL1Client>,
    config: &WriterConfig,
) -> anyhow::Result<(Transaction, Transaction)> {
    let payload = write_intent.payload();
    // let (signature, pub_key) = sign_blob_with_private_key(&payload, &config.private_key)?;
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
        &payload,
        utxos,
        config.sequencer_address.clone(),
        config.amount_for_reveal_txn,
        fee_rate,
        rpc_client.network(),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Initializes the writer state by creating a queue of transactions starting from the latest blob.
///
/// This function retrieves the latest blob and its corresponding transaction index, then creates a
/// queue of transactions by looping backwards until finalized transactions are reached. The queue
/// is then used to initialize the writer state.
///
/// # Returns
/// * `anyhow::Result<WriterState<D>>`: The initialized writer state or an error.
///
/// # Errors
/// Returns an error if fetching data from the database fails.
fn initialize_writer_state<D: SequencerDatabase>(db: Arc<D>) -> anyhow::Result<WriterState<D>> {
    if let Some(last_idx) = db.sequencer_provider().get_last_blob_idx()? {
        let blobid = db.sequencer_provider().get_blobid_for_blob_idx(last_idx)?;
        match blobid {
            Some(blobid) => {
                if let Some(txidx) = db.sequencer_provider().get_reveal_txidx_for_blob(blobid)? {
                    let (queue, start_idx) = create_txn_queue(txidx, db.clone())?;
                    return Ok(WriterState::new(db, queue, start_idx));
                }
            }
            None => {
                // TODO: what to do?
            }
        }
    }
    return Ok(WriterState::new_empty(db));
}

/// Creates a queue of transactions starting from a given reveal transaction index.
///
/// This function builds a queue of transactions by fetching commit and reveal transactions in pairs
/// starting from the specified index and working backwards until a transaction with the status
/// `Finalized` is encountered or the beginning of the sequence is reached.
///
/// # Type Parameters
/// * `D`: A type implementing the `SequencerDatabase` trait.
///
/// # Parameters
/// * `txidx`: The starting transaction index. This should probably be the latest txn idx.
/// * `db`: An `Arc` to a database implementing `SequencerDatabase`.
///
/// # Returns
/// * `DbResult<(VecDeque<TxnStatusEntry>, u64)>`: A result containing the queue of transactions and
///   the updated transaction index, or an error.
///
/// # Errors
/// Returns an error if fetching transactions from the database fails.
/// ```
fn create_txn_queue<D: SequencerDatabase>(
    txidx: u64,
    db: Arc<D>,
) -> DbResult<(VecDeque<TxnStatusEntry>, u64)> {
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
            .expect("Inconsistsent existence of transactions");

        if *reveal_txn.status() == L1TxnStatus::Finalized {
            return Ok((queue, txidx));
        } else {
            queue.push_front(reveal_txn);
            queue.push_front(commit_txn);
        }
        txidx -= 2;
    }
    return Ok((queue, 0));
}

/// Watches for inscription transactions status in bitcoin and resends if not in mempool, until they
/// are confirmed
async fn transactions_tracker_task<D: SequencerDatabase>(
    state: Arc<Mutex<WriterState<D>>>,
    rpc_client: Arc<impl L1Client>,
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
            process_queue_txn(txn, idx, &state, &rpc_client).await?;
        }
    }
}

async fn process_queue_txn<D: SequencerDatabase>(
    txn: &TxnStatusEntry,
    idx: usize,
    state: &Arc<Mutex<WriterState<D>>>,
    rpc_client: &Arc<impl L1Client>,
) -> anyhow::Result<()> {
    let status = match txn.status {
        L1TxnStatus::Unsent => {
            // NOTE: when sending errors, set it's status to unsent so that it is tried
            // again later
            if let Ok(_) = rpc_client.send_raw_transaction(txn.txn_raw.clone()).await {
                L1TxnStatus::InMempool
            } else {
                L1TxnStatus::Unsent
            }
        }

        L1TxnStatus::InMempool | L1TxnStatus::Confirmed => {
            check_confirmations_and_resend_txn(&txn, &rpc_client).await
        }

        L1TxnStatus::Finalized => L1TxnStatus::Finalized,
    };
    // Update/finalize the txn
    if status == L1TxnStatus::Finalized {
        state.lock().await.finalize_txn(idx)?;
    } else {
        state.lock().await.update_txn(idx, status)?;
    }
    Ok(())
}

async fn check_confirmations_and_resend_txn(
    txn: &TxnStatusEntry,
    rpc_client: &Arc<impl L1Client>,
) -> L1TxnStatus {
    let confs = rpc_client.get_transaction_confirmations(txn.txid().0).await;

    match confs {
        Ok(confs) if confs > FINALITY_DEPTH => {
            info!(txn = %hex::encode(txn.txid().0), "Transaction finalized");
            L1TxnStatus::Finalized
        }

        Ok(confs) if confs > 0 => L1TxnStatus::Confirmed,

        Ok(_) => L1TxnStatus::InMempool,

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
            L1TxnStatus::Unsent
        }
    }
}

#[cfg(test)]
mod test {
    use std::{str::FromStr, sync::Arc};

    use async_trait::async_trait;
    use bitcoin::{consensus::deserialize, hashes::Hash, Address, Block, BlockHash, Network, Txid};
    use tokio::sync::Mutex;

    use alpen_express_db::{sequencer::db::SequencerDB, traits::SequencerDatabase, SeqDb};
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::{
        rpc::{
            traits::{L1Client, SeqL1Client},
            types::{RawUTXO, RpcBlockchainInfo},
            ClientError,
        },
        writer::{config::WriterConfig, state::WriterState},
    };

    struct TestBitcoinClient {
        pub confs: u64,
    }
    impl TestBitcoinClient {
        fn new(confs: u64) -> Self {
            Self { confs }
        }
    }

    const TEST_BLOCKSTR: &str = "000000207d862a78fcb02ab24ebd154a20b9992af6d2f0c94d3a67b94ad5a0009d577e70769f3ff7452ea5dd469d7d99f200d083d020f1585e4bd9f52e9d66b23891a9c6c4ea5e66ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff04025f0200ffffffff02205fa01200000000160014d7340213b180c97bd55fedd7312b7e17389cf9bf0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

    #[async_trait]
    impl L1Client for TestBitcoinClient {
        async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, ClientError> {
            Ok(ArbitraryGenerator::new().generate())
        }

        // get_block_hash returns the block hash of the block at the given height
        async fn get_block_hash(&self, _h: u64) -> Result<BlockHash, ClientError> {
            let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
            Ok(block.block_hash())
        }

        async fn get_block_at(&self, _height: u64) -> Result<Block, ClientError> {
            let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
            Ok(block)
        }

        // send_raw_transaction sends a raw transaction to the network
        async fn send_raw_transaction<T: AsRef<[u8]> + Send>(
            &self,
            _tx: T,
        ) -> Result<Txid, ClientError> {
            Ok(Txid::from_slice(&[1u8; 32]).unwrap())
        }

        async fn get_transaction_confirmations<T: AsRef<[u8]> + Send>(
            &self,
            _txid: T,
        ) -> Result<u64, ClientError> {
            Ok(self.confs)
        }
    }

    #[async_trait]
    impl SeqL1Client for TestBitcoinClient {
        // get_utxos returns all unspent transaction outputs for the wallets of bitcoind
        async fn get_utxos(&self) -> Result<Vec<RawUTXO>, ClientError> {
            // Generate enough utxos to cover for the costs later
            let utxos: Vec<_> = (1..10)
                .into_iter()
                .map(|_| ArbitraryGenerator::new().generate())
                .enumerate()
                .map(|(i, x)| RawUTXO {
                    txid: hex::encode(&[i as u8; 32]), // need to do this otherwise random str is
                    // generated
                    amount: 100 * 100_000_000,
                    spendable: true,
                    solvable: true,
                    ..x
                })
                .collect();
            Ok(utxos)
        }

        async fn estimate_smart_fee(&self) -> Result<u64, ClientError> {
            Ok(3)
        }

        fn network(&self) -> Network {
            Network::Regtest
        }
    }

    fn get_db() -> Arc<SequencerDB<SeqDb>> {
        let db = alpen_test_utils::get_rocksdb_tmp_instance().unwrap();
        let seqdb = Arc::new(SeqDb::new(db));
        Arc::new(SequencerDB::new(seqdb))
    }

    fn get_config() -> WriterConfig {
        let addr = Address::from_str("bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5")
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap();
        WriterConfig {
            sequencer_address: addr,
            rollup_name: "alpen".to_string(),
            inscription_fee_policy: InscriptionFeePolicy::Fixed(100),
            poll_duration_ms: 1000,
            amount_for_reveal_txn: 1000,
        }
    }

    #[tokio::test]
    async fn test_handle_intent_new_intent() {
        let db = get_db();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        let last_idx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(last_idx, None);

        handle_intent(intent.clone(), db.clone(), &client, &config, state)
            .await
            .unwrap();

        // There should be new intent entry in db along with commit reveal txns
        assert!(db
            .sequencer_provider()
            .get_blob_by_id(intent_hash)
            .unwrap()
            .is_some());

        let reveal_idx = 1;
        assert_eq!(
            db.sequencer_provider()
                .get_reveal_txidx_for_blob(intent_hash)
                .unwrap(),
            Some(reveal_idx)
        );

        let last_idx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(last_idx, Some(0));

        let last_txn_idx = db.sequencer_provider().get_last_txn_idx().unwrap();
        assert_eq!(last_txn_idx, Some(1));
    }

    #[tokio::test]
    async fn test_handle_intent_existing_intent() {
        let db = get_db();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        // insert the intent
        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let last_idx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(last_idx, Some(0));

        handle_intent(intent.clone(), db.clone(), &client, &config, state)
            .await
            .unwrap();

        // There should be new intent entry in db along with commit reveal txns

        assert!(db
            .sequencer_provider()
            .get_blob_by_id(intent_hash)
            .unwrap()
            .is_some());

        let reveal_idx = 1;
        assert_eq!(
            db.sequencer_provider()
                .get_reveal_txidx_for_blob(intent_hash)
                .unwrap(),
            Some(reveal_idx)
        );

        let last_idx_new = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(
            last_idx, last_idx_new,
            "Idx should not change as no new blob is inserted"
        );

        let last_txn_idx = db.sequencer_provider().get_last_txn_idx().unwrap();
        assert_eq!(last_txn_idx, Some(1));
    }

    #[tokio::test]
    async fn test_handle_intent_existing_commit_reveal() {
        let db = get_db();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        // insert the intent
        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let last_idx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(last_idx, Some(0));

        // insert commit reveal for the intent
        let (commit, reveal) = create_inscriptions_from_intent(&intent, &client, &config)
            .await
            .unwrap();

        let commit_tx = TxnStatusEntry::from_txn_unsent(&commit);
        let reveal_tx = TxnStatusEntry::from_txn_unsent(&reveal);

        db.sequencer_store()
            .put_commit_reveal_txns(intent_hash, commit_tx, reveal_tx)
            .unwrap();

        handle_intent(intent.clone(), db.clone(), &client, &config, state)
            .await
            .unwrap();

        // There should be new intent entry in db along with commit reveal txns

        assert!(db
            .sequencer_provider()
            .get_blob_by_id(intent_hash)
            .unwrap()
            .is_some());

        let reveal_idx = 1;
        assert_eq!(
            db.sequencer_provider()
                .get_reveal_txidx_for_blob(intent_hash)
                .unwrap(),
            Some(reveal_idx)
        );

        let last_idx_new = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(
            last_idx, last_idx_new,
            "Idx should not change as no new blob is inserted"
        );

        let last_txn_idx = db.sequencer_provider().get_last_txn_idx().unwrap();
        assert_eq!(last_txn_idx, Some(1));
    }

    // Tests for process_queue_txn

    #[tokio::test]
    async fn test_process_queue_txn_unsent() {
        let client = Arc::new(TestBitcoinClient::new(0));
        let config = get_config();
        let db = get_db();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let (c, r) = create_inscriptions_from_intent(&intent, &client, &config)
            .await
            .unwrap();

        let rtxn = TxnStatusEntry::from_txn_unsent(&r);
        let ctxn = TxnStatusEntry::from_txn_unsent(&c);

        // insert to db
        db.sequencer_store()
            .put_commit_reveal_txns(intent_hash, ctxn.clone(), rtxn)
            .unwrap();

        // Now, the idx of ctxn in db is 0 and that of rtxn is 1. And the start_txn_idx in state is
        // 0

        // Add an new unsent txn to state
        state.lock().await.add_new_txn(ctxn.clone());

        process_queue_txn(&ctxn, 0, &state, &client).await.unwrap();

        // The txn should be InMempool now
        let st = state.lock().await;
        let txn = st.txns_queue.get(0).unwrap();
        assert_eq!(*txn.status(), L1TxnStatus::InMempool);
    }

    #[tokio::test]
    async fn test_process_queue_txn_inmempool_2_confirmations() {
        let client = Arc::new(TestBitcoinClient::new(2));
        let config = get_config();
        let db = get_db();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let (c, r) = create_inscriptions_from_intent(&intent, &client, &config)
            .await
            .unwrap();

        let rtxn = TxnStatusEntry::from_txn(&r, L1TxnStatus::InMempool);
        let ctxn = TxnStatusEntry::from_txn(&c, L1TxnStatus::InMempool);

        // insert to db
        db.sequencer_store()
            .put_commit_reveal_txns(intent_hash, ctxn.clone(), rtxn)
            .unwrap();

        // Now, the idx of ctxn in db is 0 and that of rtxn is 1. And the start_txn_idx in state is
        // 0

        // Add an new unsent txn to state
        state.lock().await.add_new_txn(ctxn.clone());

        process_queue_txn(&ctxn, 0, &state, &client).await.unwrap();

        // The txn should be Confirmed now
        let st = state.lock().await;
        let txn = st.txns_queue.get(0).unwrap();
        assert_eq!(*txn.status(), L1TxnStatus::Confirmed);
    }

    #[tokio::test]
    async fn test_process_queue_txn_inmempool_to_finalized() {
        let client = Arc::new(TestBitcoinClient::new(FINALITY_DEPTH + 1));
        let config = get_config();
        let db = get_db();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let (c, r) = create_inscriptions_from_intent(&intent, &client, &config)
            .await
            .unwrap();

        let rtxn = TxnStatusEntry::from_txn(&r, L1TxnStatus::InMempool);
        let ctxn = TxnStatusEntry::from_txn(&c, L1TxnStatus::InMempool);

        // insert to db
        db.sequencer_store()
            .put_commit_reveal_txns(intent_hash, ctxn.clone(), rtxn)
            .unwrap();

        // Now, the idx of ctxn in db is 0 and that of rtxn is 1. And the start_txn_idx in state is
        // 0

        // Add an new unsent txn to state
        state.lock().await.add_new_txn(ctxn.clone());

        process_queue_txn(&ctxn, 0, &state, &client).await.unwrap();

        // If finalized, should be removed from queue
        let st = state.lock().await;
        assert_eq!(st.txns_queue.len(), 0);
        assert_eq!(st.start_txn_idx, 1);
    }

    #[tokio::test]
    async fn test_process_queue_txn_finalized() {
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();
        let db = get_db();
        let state = Arc::new(Mutex::new(WriterState::new_empty(db.clone())));

        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        db.sequencer_store()
            .put_blob(intent_hash, intent.payload().to_vec())
            .unwrap();

        let (c, r) = create_inscriptions_from_intent(&intent, &client, &config)
            .await
            .unwrap();

        let rtxn = TxnStatusEntry::from_txn(&r, L1TxnStatus::Finalized);
        let ctxn = TxnStatusEntry::from_txn(&c, L1TxnStatus::Finalized);

        // insert to db
        db.sequencer_store()
            .put_commit_reveal_txns(intent_hash, ctxn.clone(), rtxn)
            .unwrap();

        // Now, the idx of ctxn in db is 0 and that of rtxn is 1. And the start_txn_idx in state is
        // 0

        // Add an new unsent txn to state
        state.lock().await.add_new_txn(ctxn.clone());

        process_queue_txn(&ctxn, 0, &state, &client).await.unwrap();

        // The txn should be removed from the state and the start_txn_idx should have increased by
        // 1
        let st = state.lock().await;
        assert_eq!(st.txns_queue.len(), 0);
        assert_eq!(st.start_txn_idx, 1);
    }
}
