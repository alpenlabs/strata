use std::{sync::Arc, time::Duration};

use tokio::sync::{mpsc, RwLock};
use tracing::*;

use alpen_express_db::{
    traits::SequencerDatabase,
    types::{BlobEntry, BlobL1Status},
};
use alpen_express_rpc_types::L1Status;
use express_storage::{
    managers::inscription::InscriptionManager,
    ops::inscription::{Context, InscriptionDataOps},
    BroadcastDbOps,
};
use express_tasks::TaskExecutor;

use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::traits::{L1Client, SeqL1Client},
    writer::signer::create_and_sign_blob_inscriptions,
};

use super::{config::WriterConfig, signer::start_signer_task};

#[derive(Debug)]
pub struct WriterInitialState {
    /// Next unfinalized block to watch for
    pub next_watch_blob_idx: u64,

    // Next blob idx to publish
    pub next_publish_blob_idx: u64,
}

pub fn start_inscription_tasks<D: SequencerDatabase + Send + Sync + 'static>(
    executor: &TaskExecutor,
    rpc_client: Arc<impl SeqL1Client + L1Client>,
    config: WriterConfig,
    db: Arc<D>,
    l1_status: Arc<RwLock<L1Status>>,
    pool: threadpool::ThreadPool,
    bcast_handle: Arc<L1BroadcastHandle>,
) -> anyhow::Result<InscriptionManager> {
    let (signer_tx, signer_rx) = mpsc::channel::<u64>(10);

    let ops = Arc::new(Context::new(db).into_ops(pool));
    let ops_s = ops.clone();
    let ops_w = ops.clone();
    let insc_mgr = InscriptionManager::new(ops, signer_tx);

    let WriterInitialState {
        next_watch_blob_idx,
        ..
    } = initialize_writer_state(ops_s.as_ref())?;

    // The watcher task watches L1 for txs confirmations and finalizations. Ideally this should be
    // taken care of by the reader task. This can be done later.
    let rpc_client_w = rpc_client.clone();
    let config_w = config.clone();
    let bcast_ops = bcast_handle.ops();
    let bcast_ops_w = bcast_ops.clone();
    let bcast_ops_s = bcast_ops.clone();
    executor.spawn_critical_async("btcio::watcher_task", async move {
        watcher_task(
            next_watch_blob_idx,
            rpc_client_w,
            config_w,
            ops_w,
            bcast_ops_w,
        )
        .await
        .unwrap()
    });

    executor.spawn_critical_async("btcio::listen_for_signing_intents", async {
        start_signer_task(signer_rx, rpc_client, config, ops_s, bcast_ops_s)
            .await
            .unwrap()
    });
    Ok(insc_mgr)
}

fn initialize_writer_state(insc_ops: &InscriptionDataOps) -> anyhow::Result<WriterInitialState> {
    let mut next_idx = insc_ops.get_next_blob_idx_blocking()?;
    if next_idx == 0 {
        return Ok(WriterInitialState {
            next_watch_blob_idx: 0,
            next_publish_blob_idx: 0,
        });
    }

    let mut next_publish_idx = None;
    let mut next_watch_idx = 0;

    while next_idx > 0 {
        let Some(blob) = insc_ops.get_blob_entry_by_idx_blocking(next_idx - 1)? else {
            break;
        };
        match blob.status {
            // We are watching from the latest so we don't need to update next_publish_idx if we
            // found one already
            BlobL1Status::Published if next_publish_idx.is_none() => {
                next_publish_idx = Some(next_idx);
            }
            BlobL1Status::Finalized => {
                next_watch_idx = next_idx;
                // We don't need to check beyond finalized blob
                break;
            }
            _ => {}
        };
        next_idx -= 1;
    }
    Ok(WriterInitialState {
        next_watch_blob_idx: next_watch_idx,
        next_publish_blob_idx: next_publish_idx.unwrap_or(0),
    })
}

const FINALITY_DEPTH: u64 = 6;

/// Watches for inscription transactions status in bitcoin
pub async fn watcher_task(
    next_to_watch: u64,
    rpc_client: Arc<impl L1Client + SeqL1Client>,
    config: WriterConfig,
    insc_ops: Arc<InscriptionDataOps>,
    bcast_ops: Arc<BroadcastDbOps>,
) -> anyhow::Result<()> {
    info!("Starting L1 writer's watcher task");
    let interval = tokio::time::interval(Duration::from_millis(config.poll_duration_ms));
    tokio::pin!(interval);

    let mut curr_blobidx = next_to_watch;
    loop {
        interval.as_mut().tick().await;

        if let Some(blobentry) = insc_ops.get_blob_entry_by_idx_async(curr_blobidx).await? {
            match blobentry.status {
                BlobL1Status::Published | BlobL1Status::Confirmed => {
                    debug!(%curr_blobidx, "blobentry is published or confirmed");
                }
                BlobL1Status::Unsigned | BlobL1Status::NeedsResign => {
                    debug!(%curr_blobidx, "blobentry is unsigned or needs resign");
                    create_and_sign_blob_inscriptions(
                        curr_blobidx,
                        insc_ops.as_ref(),
                        bcast_ops.as_ref(),
                        rpc_client.clone(),
                        &config,
                    )
                    .await?;
                }
                BlobL1Status::Finalized => {
                    debug!(%curr_blobidx, "blobentry is finalized");
                    curr_blobidx += 1;
                }
                BlobL1Status::Unpublished => {
                    debug!(%curr_blobidx, "blobentry is unpublished;");
                } // Do Nothing
            }
        } else {
            // No blob exists, just continue the loop and thus wait for blob to be present in db
        }
    }
}

#[cfg(test)]
mod test {
    use std::{str::FromStr, sync::Arc};

    use alpen_express_primitives::buf::Buf32;
    use bitcoin::{Address, Network};

    use alpen_express_db::traits::SequencerDatabase;
    use alpen_express_rocksdb::{
        sequencer::db::SequencerDB, test_utils::get_rocksdb_tmp_instance, SeqDb,
    };

    use alpen_test_utils::ArbitraryGenerator;

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
