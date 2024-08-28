use std::sync::Arc;

use alpen_express_btcio::{
    reader::{messages::L1Event, query::bitcoin_data_reader_task},
    rpc::traits::BitcoinReader,
};
use alpen_express_consensus_logic::{ctl::CsmController, l1_handler::bitcoin_data_handler_task};
use alpen_express_db::traits::{Database, L1DataProvider};
use alpen_express_primitives::params::Params;
use alpen_express_status::StatusTx;
use express_tasks::TaskExecutor;
use tokio::sync::mpsc;

use crate::config::Config;

pub fn start_reader_tasks<D: Database + Send + Sync + 'static>(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl BitcoinReader>,
    db: Arc<D>,
    csm_ctl: Arc<CsmController>,
    status_rx: Arc<StatusTx>,
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

    let reader_config = Arc::new(config.get_reader_config());
    let params_r = params.clone();
    let chprov = db.chainstate_provider().clone();

    // TODO set up watchdog to handle when the spawned tasks fail gracefully
    executor.spawn_critical_async(
        "bitcoin_data_reader_task",
        bitcoin_data_reader_task::<D>(
            rpc_client,
            ev_tx,
            target_next_block,
            reader_config,
            status_rx.clone(),
            chprov,
            params_r,
        ),
    );

    let l1db = db.l1_store().clone();
    let _sedb = db.sync_event_store().clone();
    executor.spawn_critical("bitcoin_data_handler_task", move |_| {
        bitcoin_data_handler_task::<D>(l1db, csm_ctl, ev_rx, params).unwrap()
    });
    Ok(())
}
