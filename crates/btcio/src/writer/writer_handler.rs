use std::sync::Arc;

use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{BlobEntry, BlobL1Status},
};
use alpen_express_primitives::buf::Buf32;
use alpen_express_state::da_blob::BlobIntent;
use alpen_express_status::StatusTx;
use express_tasks::TaskExecutor;
use tokio::{
    runtime::Runtime,
    sync::{
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
};
use tracing::*;

use super::{
    broadcast::broadcaster_task,
    config::WriterConfig,
    utils::{create_and_sign_blob_inscriptions, get_blob_by_id, put_blob, BlobIdx},
};
use crate::{
    rpc::traits::{L1Client, SeqL1Client},
    writer::watcher::watcher_task,
};

#[derive(Debug)]
pub struct WriterInitialState {
    /// Next unfinalized block to watch for
    pub next_watch_blob_idx: u64,

    // Next blob idx to publish
    pub next_publish_blob_idx: u64,
}

#[derive(Clone, Debug)]
pub struct DaWriter<D> {
    db: Arc<D>,
    signer_tx: Sender<BlobIdx>,
}

impl<D: SequencerDatabase + Send + Sync + 'static> DaWriter<D> {
    pub fn submit_intent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        // Write to db and if not already exisging, notify signer about the new entry
        // if let Some(idx) = store_entry(*intent.commitment(), entry, self.db.clone())? {
        if let Some(idx) = store_entry(*intent.commitment(), entry, self.db.clone())? {
            self.signer_tx.blocking_send(idx)?;
        } // None means duplicate intent
        Ok(())
    }

    pub async fn submit_intent_async(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        // Write to db and if not already exisging, notify signer about the new entry
        if let Some(idx) = store_entry_async(*intent.commitment(), entry, self.db.clone()).await? {
            self.signer_tx.send(idx).await?;
        } // None means duplicate intent
        Ok(())
    }
}

pub fn start_writer_task<D: SequencerDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
    status_tx: Arc<StatusTx>,
) -> anyhow::Result<DaWriter<D>> {
    info!("Starting writer control task");

    let (signer_tx, signer_rx) = mpsc::channel::<BlobIdx>(10);

    let WriterInitialState {
        next_watch_blob_idx,
        next_publish_blob_idx,
    } = initialize_writer_state(db.clone())?;

    // The watcher task watches L1 for txs confirmations and finalizations. Ideally this should be
    // taken care of by the reader task. This can be done later.
    let rpc_client_w = rpc_client.clone();
    let config_w = config.clone();
    let db_w = db.clone();
    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(next_watch_blob_idx, rpc_client_w, config_w, db_w)
            .await
            .unwrap()
    });

    let rpc_client_b = rpc_client.clone();
    let db_b = db.clone();
    executor.spawn_critical_async("btcio::broadcaster_task", async move {
        broadcaster_task(next_publish_blob_idx, rpc_client_b, db_b, status_tx.clone())
            .await
            .unwrap()
    });

    let db_s = db.clone();
    executor.spawn_critical_async("btcio::listen_for_signing_intents", async {
        listen_for_signing_intents(signer_rx, rpc_client, config, db_s)
            .await
            .unwrap()
    });

    Ok(DaWriter { signer_tx, db })
}

async fn listen_for_signing_intents<D>(
    mut sign_rx: Receiver<BlobIdx>,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
) -> anyhow::Result<()>
where
    D: SequencerDatabase + Sync + Send + 'static,
{
    loop {
        let Some(blobidx) = sign_rx.recv().await else {
            break;
        };
        debug!(%blobidx, "Receicved blob for signing");

        if let Err(e) =
            create_and_sign_blob_inscriptions(blobidx, db.clone(), rpc_client.clone(), &config)
                .await
        {
            error!(%e, %blobidx, "Failed to handle blob intent");
        } else {
            debug!(%blobidx, "Successfully signed blob");
        }
    }
    Ok(())
}

fn store_entry<D: SequencerDatabase>(
    commitment: Buf32,
    entry: BlobEntry,
    db: Arc<D>,
) -> anyhow::Result<Option<u64>> {
    match db.sequencer_provider().get_blob_by_id(commitment)? {
        Some(_) => {
            warn!("duplicate write intent {commitment:?}. Ignoring");
            Ok(None)
        }
        None => {
            // Store in db
            let idx = db.sequencer_store().put_blob(commitment, entry)?;
            Ok(Some(idx))
        }
    }
}

async fn store_entry_async<D: SequencerDatabase + Send + Sync + 'static>(
    commitment: Buf32,
    entry: BlobEntry,
    db: Arc<D>,
) -> anyhow::Result<Option<u64>> {
    match get_blob_by_id(db.clone(), commitment).await? {
        Some(_) => {
            warn!("duplicate write intent {commitment:?}. Ignoring");
            Ok(None)
        }
        None => {
            // Store in db
            let idx = put_blob(db, commitment, entry).await?;
            Ok(Some(idx))
        }
    }
}

fn initialize_writer_state<D: SequencerDatabase>(db: Arc<D>) -> anyhow::Result<WriterInitialState> {
    let prov = db.sequencer_provider();

    let mut curr_idx = match prov.get_last_blob_idx()? {
        Some(idx) => idx,
        None => {
            return Ok(WriterInitialState {
                next_publish_blob_idx: 0,
                next_watch_blob_idx: 0,
            });
        }
    };

    let mut next_publish_idx = None;
    let mut next_watch_idx = 0;

    loop {
        let Some(blob) = prov.get_blob_by_idx(curr_idx)? else {
            break;
        };
        match blob.status {
            // We are watching from the latest so we don't need to update next_publish_idx if we
            // found one already
            BlobL1Status::Published if next_publish_idx.is_none() => {
                next_publish_idx = Some(curr_idx + 1);
            }
            BlobL1Status::Finalized => {
                next_watch_idx = curr_idx + 1;
                // We don't need to check beyond finalized blob
                break;
            }
            _ => {}
        };
        if curr_idx == 0 {
            break;
        }
        curr_idx -= 1;
    }
    Ok(WriterInitialState {
        next_watch_blob_idx: next_watch_idx,
        next_publish_blob_idx: next_publish_idx.unwrap_or(0),
    })
}

#[cfg(test)]
mod test {
    use std::{str::FromStr, sync::Arc};

    use alpen_express_db::traits::SequencerDatabase;
    use alpen_express_primitives::buf::Buf32;
    use alpen_express_rocksdb::{
        sequencer::db::SequencerDB, test_utils::get_rocksdb_tmp_instance, SeqDb,
    };
    use alpen_test_utils::ArbitraryGenerator;
    use bitcoin::{Address, Network};

    use super::*;
    use crate::writer::config::{InscriptionFeePolicy, WriterConfig};

    fn get_db() -> Arc<SequencerDB<SeqDb>> {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seqdb = Arc::new(SeqDb::new(db, db_ops));
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

    #[test]
    fn test_initialize_writer_state_no_last_blob_idx() {
        let db = get_db();

        let lastidx = db.sequencer_provider().get_last_blob_idx().unwrap();
        assert_eq!(lastidx, None);

        let st = initialize_writer_state(db.clone()).unwrap();

        assert_eq!(st.next_publish_blob_idx, 0);
        assert_eq!(st.next_watch_blob_idx, 0);
    }

    #[test]
    fn test_initialize_writer_state_with_existing_blobs() {
        let db = get_db();

        let mut e1: BlobEntry = ArbitraryGenerator::new().generate();
        e1.status = BlobL1Status::Finalized;
        let blob_hash: Buf32 = [1; 32].into();
        let _idx1 = db.sequencer_store().put_blob(blob_hash, e1).unwrap();

        let mut e2: BlobEntry = ArbitraryGenerator::new().generate();
        e2.status = BlobL1Status::Published;
        let blob_hash: Buf32 = [2; 32].into();
        let idx2 = db.sequencer_store().put_blob(blob_hash, e2).unwrap();

        let mut e3: BlobEntry = ArbitraryGenerator::new().generate();
        e3.status = BlobL1Status::Unsigned;
        let blob_hash: Buf32 = [3; 32].into();
        let idx3 = db.sequencer_store().put_blob(blob_hash, e3).unwrap();

        let mut e4: BlobEntry = ArbitraryGenerator::new().generate();
        e4.status = BlobL1Status::Unsigned;
        let blob_hash: Buf32 = [4; 32].into();
        let _idx4 = db.sequencer_store().put_blob(blob_hash, e4).unwrap();

        let st = initialize_writer_state(db.clone()).unwrap();

        assert_eq!(st.next_watch_blob_idx, idx2);
        assert_eq!(st.next_publish_blob_idx, idx3);
    }
}
