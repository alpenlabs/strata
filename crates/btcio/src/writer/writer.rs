use std::{collections::HashMap, sync::Arc, time::Duration};

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::da_blob::{BlobDest, BlobIntent};
use anyhow::anyhow;
use bitcoin::{consensus::serialize, hashes::Hash, Transaction};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::*;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{BlobEntry, BlobL1Status},
};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::da_blob::{BlobDest, BlobIntent};

use super::utils::{get_blob_by_idx, update_blob_by_idx};
use super::{broadcast::broadcaster_task, builder::build_inscription_txs};
use super::{config::WriterConfig, state::WriterState};
use crate::rpc::traits::{L1Client, SeqL1Client};

const FINALITY_DEPTH: u64 = 6;

pub struct DaWriter {
    intent_tx: Sender<BlobIntent>,
}

impl DaWriter {
    pub fn submit_inent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        Ok(self.intent_tx.blocking_send(intent)?)
    }
}

pub fn start_writer_task<D: SequencerDatabase + Send + Sync + 'static>(
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<DaWriter> {
    info!("Starting writer control task");

    let (intent_tx, intent_rx) = mpsc::channel::<BlobIntent>(10);

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

    tokio::spawn(listen_for_write_intents(intent_rx, rpc_client, config, db));

    Ok(DaWriter { intent_tx })
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

        if let Err(e) = handle_intent(write_intent, db.clone(), rpc_client.clone(), &config).await {
            error!(%e, "Failed to handle intent");
        }
    }
}

/// This returns None if the intent already exists and returns Some(commit_txid, reveal_txid) for
/// new intent
async fn handle_intent<D: SequencerDatabase + Send + Sync + 'static>(
    intent: BlobIntent,
    db: Arc<D>,
    client: Arc<impl L1Client + SeqL1Client>,
    config: &WriterConfig,
) -> anyhow::Result<Option<(Buf32, Buf32)>> {
    // If it is already present in the db and corresponding txns are created, return

    // TODO: handle insufficient utxos
    let (commit, reveal) = build_inscription_txs(&intent.payload(), &client, &config).await?;

    tokio::task::spawn_blocking(move || store_intent(intent, db, commit, reveal)).await?
}

fn store_intent<D: SequencerDatabase + Send + Sync + 'static>(
    intent: BlobIntent,
    db: Arc<D>,
    commit: Transaction,
    reveal: Transaction,
) -> anyhow::Result<Option<(Buf32, Buf32)>> {
    let blobid = calculate_intent_hash(&intent.payload()); // TODO: should we use the commitment in
                                                           // the intent?
    match db.sequencer_provider().get_blob_by_id(blobid.clone())? {
        Some(_) => {
            warn!("duplicate write intent {blobid:?}. Ignoring");
            return Ok(None);
        }
        None => {
            // Store in db
            let cid: Buf32 = commit.compute_txid().as_raw_hash().to_byte_array().into();
            let rid: Buf32 = reveal.compute_txid().as_raw_hash().to_byte_array().into();

            db.sequencer_store().put_commit_reveal_txs(
                cid,
                serialize(&commit),
                rid,
                serialize(&reveal),
            )?;

            let blobentry = BlobEntry::new_unsent(intent.payload().to_vec(), cid, rid);
            let _ = db.sequencer_store().put_blob(blobid, blobentry)?;

            return Ok(Some((cid, rid)));
        }
    }
}

fn calculate_intent_hash(intent: &[u8]) -> Buf32 {
    let hash: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(intent);
        hasher.finalize().into()
    };
    Buf32(hash.into())
}

fn initialize_writer_state<D: SequencerDatabase>(db: Arc<D>) -> anyhow::Result<WriterState<D>> {
    let prov = db.sequencer_provider();

    let last_idx = match prov.get_last_blob_idx()? {
        Some(last_idx) => last_idx,
        None => {
            // Insert sentinel blob to db and return.
            // Sentinel here is just empty. And the major purpose is to have cleaner data structures
            // instead of dealing with Option<u64> in states.
            let entry = BlobEntry::new(
                Default::default(),
                Buf32::zero(),
                Buf32::zero(),
                BlobL1Status::Finalized,
            );
            let idx = db.sequencer_store().put_blob(Buf32::zero(), entry)?;
            assert_eq!(idx, 0, "Expect sentinel blobid to be zero");
            return Ok(WriterState::new(db, 0, 0));
        }
    };

    let mut curr_idx = last_idx;

    let mut last_sent_idx = 0;
    let mut last_finalized_idx = 0;

    while curr_idx > 0 {
        let Some(blob) = prov.get_blob_by_idx(curr_idx)? else {
            break;
        };
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

        if let Some(mut blobentry) = get_blob_by_idx(db.clone(), curr_blobidx).await? {
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
                update_blob_by_idx(db.clone(), curr_blobidx, blobentry.clone()).await?;

                // Also set the blob that is deep enough as finalized
                let finidx = curr_blobidx - FINALITY_DEPTH;
                if let Some(blobentry) = blobentries.get(&finidx) {
                    let mut blobentry = blobentry.clone();
                    blobentry.status = BlobL1Status::Finalized;
                    blobentries.insert(finidx, blobentry.clone());
                    update_blob_by_idx(db.clone(), finidx, blobentry.clone()).await?;
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

    use alpen_express_db::{sequencer::db::SequencerDB, traits::SequencerDatabase, SeqDb};
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::{
        rpc::{
            traits::{L1Client, SeqL1Client},
            types::{RawUTXO, RpcBlockchainInfo},
            ClientError,
        },
        writer::config::{InscriptionFeePolicy, WriterConfig},
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
    async fn test_handle_intent() {
        let db = get_db();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();

        // TODO: Explicitly make sure the intent dest is L1
        let intent: BlobIntent = ArbitraryGenerator::new().generate();
        let intent_hash = calculate_intent_hash(&intent.payload());

        let last_idx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(last_idx, None);

        // Assert there's no blobentry in db
        assert_eq!(
            db.sequencer_provider().get_blob_by_id(intent_hash).unwrap(),
            None
        );

        let res = handle_intent(intent.clone(), db.clone(), client.clone(), &config)
            .await
            .unwrap();
        assert!(res.is_some());

        let (cid, rid) = res.unwrap();

        // There should be new intent entry in db along with commit reveal txns
        assert!(db
            .sequencer_provider()
            .get_blob_by_id(intent_hash)
            .unwrap()
            .is_some());

        // There should be txns in db as well
        assert!(db.sequencer_provider().get_l1_tx(cid).unwrap().is_some());
        assert!(db.sequencer_provider().get_l1_tx(rid).unwrap().is_some());

        // Now if the intent is handled again, it should return None
        let res = handle_intent(intent.clone(), db.clone(), client, &config)
            .await
            .unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_initialize_writer_state_no_last_blob_idx() {
        let db = get_db();

        let blb = db.sequencer_provider().get_blob_by_idx(0);
        assert!(blb.is_err());

        let st = initialize_writer_state(db.clone()).unwrap();

        let blb = db.sequencer_provider().get_blob_by_idx(0).unwrap();
        assert!(
            blb.is_some(),
            "There should be initial blob after initialization"
        );

        assert_eq!(st.last_sent_blob_idx, 0);
        assert_eq!(st.last_finalized_blob_idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_unsent_blobs() {
        let db = get_db();

        let mut e1: BlobEntry = ArbitraryGenerator::new().generate();
        e1.status = BlobL1Status::Finalized;
        let blob_hash: Buf32 = [1; 32].into();
        let idx1 = db.sequencer_store().put_blob(blob_hash, e1).unwrap();

        let mut e2: BlobEntry = ArbitraryGenerator::new().generate();
        e2.status = BlobL1Status::Confirmed;
        let blob_hash: Buf32 = [2; 32].into();
        let idx2 = db.sequencer_store().put_blob(blob_hash, e2).unwrap();

        let mut e3: BlobEntry = ArbitraryGenerator::new().generate();
        e3.status = BlobL1Status::InMempool;
        let blob_hash: Buf32 = [3; 32].into();
        let idx3 = db.sequencer_store().put_blob(blob_hash, e3).unwrap();

        let mut e4: BlobEntry = ArbitraryGenerator::new().generate();
        e4.status = BlobL1Status::Unsent;
        let blob_hash: Buf32 = [4; 32].into();
        let idx4 = db.sequencer_store().put_blob(blob_hash, e4).unwrap();

        let st = initialize_writer_state(db.clone()).unwrap();

        assert_eq!(st.last_sent_blob_idx, idx3);
        assert_eq!(st.last_finalized_blob_idx, idx1);
    }
}
