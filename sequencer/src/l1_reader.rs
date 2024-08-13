use std::sync::Arc;
use std::thread;

use tokio::sync::{mpsc, RwLock};

use alpen_express_btcio::reader::{
    config::ReaderConfig, messages::L1Event, query::bitcoin_data_reader_task,
};
use alpen_express_btcio::rpc::traits::L1Client;
use alpen_express_consensus_logic::ctl::CsmController;
use alpen_express_consensus_logic::l1_handler::bitcoin_data_handler_task;
use alpen_express_db::traits::{Database, L1DataProvider};
use alpen_express_primitives::params::Params;
use alpen_express_rpc_types::L1Status;

use crate::config::Config;

pub async fn start_reader_tasks<D: Database>(
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl L1Client>,
    db: Arc<D>,
    csm_ctl: Arc<CsmController>,
    l1_status: Arc<RwLock<L1Status>>,
) -> anyhow::Result<()>
where
    // TODO how are these not redundant trait bounds???
    <D as alpen_express_db::traits::Database>::SeStore: Send + Sync + 'static,
    <D as alpen_express_db::traits::Database>::L1Store: Send + Sync + 'static,
{
    let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

    // TODO switch to checking the L1 tip in the consensus/client state
    let l1prov = db.l1_provider().clone();
    let target_next_block = l1prov
        .get_chain_tip()?
        .map(|i| i + 1)
        .unwrap_or(params.rollup().horizon_l1_height);

    let config = Arc::new(ReaderConfig {
        max_reorg_depth: config.sync.max_reorg_depth,
        client_poll_dur_ms: config.sync.client_poll_dur_ms,
    });

    // TODO set up watchdog to handle when the spawned tasks fail gracefully
    let _reader_handle = tokio::spawn(bitcoin_data_reader_task(
        rpc_client,
        ev_tx,
        target_next_block,
        config.clone(),
        l1_status.clone(),
    ));

    let l1db = db.l1_store().clone();
    let _sedb = db.sync_event_store().clone();
    let _handler_handle =
        thread::spawn(move || bitcoin_data_handler_task(l1db, csm_ctl, ev_rx, params));
    Ok(())
}
