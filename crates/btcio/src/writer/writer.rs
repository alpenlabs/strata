use std::{collections::HashMap, sync::Arc, time::Duration};

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::da_blob::{BlobDest, BlobIntent};
use anyhow::anyhow;
use bitcoin::Transaction;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::Receiver;
use tracing::*;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{BlobEntry, BlobL1Status},
};

use super::{
    builder::{create_inscription_transactions, UtxoParseError, UTXO},
    config::{InscriptionFeePolicy, WriterConfig},
    state::WriterState,
};
use crate::rpc::{
    traits::{L1Client, SeqL1Client},
    types::RawUTXO,
    ClientError,
};

const FINALITY_DEPTH: u64 = 6;
const BROADCAST_POLL_INTERVAL: u64 = 5000; // millis

pub async fn writer_control_task<D: SequencerDatabase + Send + Sync + 'static>(
    intent_rx: Receiver<BlobIntent>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
    info!("Starting writer control task");

    let state = initialize_writer_state(db.clone())?;

    // The watcher task watches L1 for txs confirmations and finalizations. Ideally this should be
    // taken care of by the reader task. This can be done later.
    tokio::spawn(watcher_task(
        state.last_finalized_blob_idx,
        rpc_client.clone(),
        config.clone(),
        db.clone(),
    ));

    tokio::spawn(broadcaster_task(
        state,
        rpc_client.clone(),
        config.clone(),
        db.clone(),
    ));

    let _ = listen_for_write_intents(intent_rx, rpc_client, config, db).await;

    Ok(())
}

async fn listen_for_write_intents<D>(
    mut intent_rx: Receiver<BlobIntent>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
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

        if let Err(e) = handle_intent(write_intent, db.clone(), &rpc_client, &config).await {
            error!(%e, "Failed to handle intent");
        }
    }
}

async fn handle_intent<D: SequencerDatabase>(
    intent: BlobIntent,
    db: Arc<D>,
    rpc_client: &Arc<impl SeqL1Client + L1Client>,
    config: &WriterConfig,
) -> anyhow::Result<()> {
    // If it is already present in the db and corresponding txns are created, return

    let blobid = calculate_intent_hash(&intent.payload()); // TODO: should we use the commitment
                                                           // here?
    let seqprov = db.sequencer_provider();
    let seqstore = db.sequencer_store();

    match seqprov.get_blob_by_id(blobid.clone())? {
        Some(_) => {
            warn!("duplicate write intent {blobid:?}. Ignoring");
            return Ok(());
        }
        None => {
            // Store in db
            // TODO: handle insufficient utxos
            let (commit, reveal) =
                create_inscriptions_from_intent(&intent.payload(), &rpc_client, &config).await?;
            let (commit_txid, reveal_txid) = seqstore.put_commit_reveal_txs(commit, reveal)?;
            let blobentry =
                BlobEntry::new_unsent(intent.payload().to_vec(), commit_txid, reveal_txid);
            let _ = seqstore.put_blob(blobid, blobentry)?;
        }
    }
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
    payload: &[u8],
    rpc_client: &Arc<impl SeqL1Client + L1Client>,
    config: &WriterConfig,
) -> anyhow::Result<(Transaction, Transaction)> {
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

fn initialize_writer_state<D: SequencerDatabase>(db: Arc<D>) -> anyhow::Result<WriterState<D>> {
    let prov = db.sequencer_provider();

    let last_idx = match prov.get_last_blob_idx()? {
        Some(last_idx) => last_idx,
        None => {
            // Insert genesis blob to db and return.
            // Genesis here is just empty. And the major purpose is to have something instead of
            // dealing with Option<u64> in states.
            let entry = BlobEntry::new(
                Default::default(),
                Buf32::zero(),
                Buf32::zero(),
                BlobL1Status::Finalized,
            );
            let idx = db.sequencer_store().put_blob(Buf32::zero(), entry)?;
            assert_eq!(idx, 0, "Expect genesis blobid to be zero");
            return Ok(WriterState::new(db, 0, 0));
        }
    };

    let mut curr_idx = last_idx;

    let mut last_sent_idx = 0;
    let mut last_finalized_idx = 0;

    loop {
        let blob = if let Some(blob) = prov.get_blob_by_idx(curr_idx)? {
            blob
        } else {
            break;
        };
        if curr_idx <= 0 {
            break;
        }
        match blob.status {
            BlobL1Status::InMempool => {
                last_sent_idx = curr_idx;
            }
            BlobL1Status::Finalized => {
                last_finalized_idx = curr_idx;
                // We don't need to check beyond finalized blob
                break;
            }
            _ => {}
        };
        curr_idx -= 1;
    }
    return Ok(WriterState::new(db, last_finalized_idx, last_sent_idx));
}

/// Broadcasts the next blob to be sent
async fn broadcaster_task<D: SequencerDatabase + Send + Sync + 'static>(
    mut state: WriterState<D>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
    let interval = tokio::time::interval(Duration::from_millis(BROADCAST_POLL_INTERVAL));
    tokio::pin!(interval);

    loop {
        // SLEEP!
        interval.as_mut().tick().await;

        // Check from db if the last sent is confirmed because if we sent the new one before the
        // previous is confirmed, they might end up in different order
        if db
            .sequencer_provider()
            .get_blob_by_idx(state.last_sent_blob_idx)?
            .map(|x| x.status == BlobL1Status::Confirmed)
            .ok_or(anyhow!("Last sent blob not found in db"))?
        {
            continue;
        }

        let next_idx = state.last_sent_blob_idx + 1;

        if let Some(blobentry) = db
            .sequencer_provider()
            .get_blob_by_idx(state.last_sent_blob_idx)?
        {
            // Get commit reveal txns
            let commit_tx = db
                .sequencer_provider()
                .get_l1_tx(blobentry.commit_txid)?
                .ok_or(anyhow!("Expected to find commit tx in db"))?;
            let reveal_tx = db
                .sequencer_provider()
                .get_l1_tx(blobentry.reveal_txid)?
                .ok_or(anyhow!("Expected to find reveal tx in db"))?;
            // Send
            match send_commit_reveal_txs(commit_tx, reveal_tx, rpc_client.as_ref()).await {
                Ok(_) => {
                    state.last_sent_blob_idx = next_idx;
                }
                Err(SendError::MissingOrInvalidInput) => {
                    // This is tricky, need to reconstruct commit-reveal txns. Might need to resend
                    // previous ones as well.
                    let (commit, reveal) =
                        create_inscriptions_from_intent(&blobentry.blob, &rpc_client, &config)
                            .await?;
                    let (commit_txid, reveal_txid) =
                        db.sequencer_store().put_commit_reveal_txs(commit, reveal)?;
                    let new_blobentry =
                        BlobEntry::new_unsent(blobentry.blob.clone(), commit_txid, reveal_txid);

                    db.sequencer_store()
                        .update_blob_by_idx(state.last_sent_blob_idx, new_blobentry)?;
                    // Do nothing, this will be sent in the next step of the loop
                }
                Err(SendError::Other(errstr)) => {
                    // TODO: Maybe retry?
                }
            }
        }
    }
}

enum SendError {
    MissingOrInvalidInput,
    Other(String),
}

async fn send_commit_reveal_txs(
    commit_tx_raw: Vec<u8>,
    reveal_tx_raw: Vec<u8>,
    client: &(impl SeqL1Client + L1Client),
) -> Result<(), SendError> {
    match send_tx(commit_tx_raw, client).await {
        Ok(_) => send_tx(reveal_tx_raw, client).await,
        Err(e) => Err(e),
    }
}

async fn send_tx(tx_raw: Vec<u8>, client: &(impl SeqL1Client + L1Client)) -> Result<(), SendError> {
    match client.send_raw_transaction(tx_raw).await {
        Ok(_) => Ok(()),
        Err(ClientError::Server(-27, _)) => Ok(()), // Tx already in chain
        Err(ClientError::Server(-26, _)) => Err(SendError::MissingOrInvalidInput),
        Err(e) => Err(SendError::Other(e.to_string())),
    }
}

/// Watches for inscription transactions status in bitcoin
async fn watcher_task<D: SequencerDatabase + Send + Sync + 'static>(
    last_finalized_blob_idx: u64,
    rpc_client: Arc<impl L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()> {
    // TODO: better interval values and placement in the loop
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    let mut curr_blobidx = last_finalized_blob_idx + 1; // start with the next blob
    let mut blobentries: HashMap<u64, BlobEntry> = HashMap::new();
    loop {
        interval.as_mut().tick().await;

        if let Some(mut blobentry) = db.sequencer_provider().get_blob_by_idx(curr_blobidx)? {
            let confs = rpc_client
                .get_transaction_confirmations(blobentry.reveal_txid.0)
                .await?;
            // If confs is 0 then it is probably in mempool
            // TODO: But if confs is error(saying txn not found, TODO: check this) then it could
            // possibly have reorged

            if confs >= 1 {
                // blob is confirmed, mark it as confirmed
                blobentry.status = BlobL1Status::Confirmed;
                blobentries.insert(curr_blobidx, blobentry.clone());

                // Update this in db
                db.sequencer_store()
                    .update_blob_by_idx(curr_blobidx, blobentry.clone())?;

                // Also set the blob that is deep enough to be finalized
                let finidx = curr_blobidx - FINALITY_DEPTH;
                if let Some(blobentry) = blobentries.get(&finidx) {
                    let mut blobentry = blobentry.clone();
                    blobentry.status = BlobL1Status::Finalized;
                    blobentries.insert(finidx, blobentry.clone());
                    db.sequencer_store().update_blob_by_idx(finidx, blobentry)?;
                }
                curr_blobidx += 1;
            }
            blobentries.insert(curr_blobidx, blobentry.clone());
        } else {
            // No blob exists, just continue the loop and thus wait for blob to be present in db
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

        let commit_tx = TxEntry::from_txn_unsent(&commit);
        let reveal_tx = TxEntry::from_txn_unsent(&reveal);

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

        let rtxn = TxEntry::from_txn_unsent(&r);
        let ctxn = TxEntry::from_txn_unsent(&c);

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

        let rtxn = TxEntry::from_txn(&r, L1TxnStatus::InMempool);
        let ctxn = TxEntry::from_txn(&c, L1TxnStatus::InMempool);

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

        let rtxn = TxEntry::from_txn(&r, L1TxnStatus::InMempool);
        let ctxn = TxEntry::from_txn(&c, L1TxnStatus::InMempool);

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

        let rtxn = TxEntry::from_txn(&r, L1TxnStatus::Finalized);
        let ctxn = TxEntry::from_txn(&c, L1TxnStatus::Finalized);

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
