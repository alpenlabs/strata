use std::sync::Arc;
use std::thread;

use tokio::sync::mpsc;

use alpen_vertex_btcio::btcio_status::BtcioEvent;
use alpen_vertex_btcio::reader::{
    config::ReaderConfig, messages::L1Event, query::bitcoin_data_reader_task,
};
use alpen_vertex_btcio::rpc::traits::L1Client;
use alpen_vertex_consensus_logic::ctl::CsmController;
use alpen_vertex_consensus_logic::l1_handler::bitcoin_data_handler_task;
use alpen_vertex_db::traits::{Database, L1DataProvider};
use alpen_vertex_primitives::params::Params;

pub async fn start_reader_tasks<D: Database>(
    params: &Params,
    rpc_client: impl L1Client,
    db: Arc<D>,
    csm_ctl: Arc<CsmController>,
    l1_status_tx: mpsc::Sender<BtcioEvent>,
) -> anyhow::Result<()>
where
    // TODO how are these not redundant trait bounds???
    <D as alpen_vertex_db::traits::Database>::SeStore: Send + Sync + 'static,
    <D as alpen_vertex_db::traits::Database>::L1Store: Send + Sync + 'static,
{
    let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

    // TODO switch to checking the L1 tip in the consensus/client state
    let l1prov = db.l1_provider().clone();
    let current_block_height = l1prov
        .get_chain_tip()?
        .unwrap_or(params.rollup().l1_start_block_height - 1);

    let config = Arc::new(ReaderConfig::default());

    // TODO set up watchdog to handle when the spawned tasks fail gracefully
    let _reader_handle = tokio::spawn(bitcoin_data_reader_task(
        rpc_client,
        ev_tx,
        current_block_height,
        config.clone(),
        l1_status_tx.clone(),
    ));

    let l1db = db.l1_store().clone();
    let _sedb = db.sync_event_store().clone();
    let _handler_handle = thread::spawn(move || bitcoin_data_handler_task(l1db, csm_ctl, ev_rx, l1_status_tx.clone()));
    Ok(())
}
