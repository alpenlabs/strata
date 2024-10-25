use std::sync::Arc;

use strata_btcio::{reader::query::bitcoin_data_reader_task, rpc::traits::Reader};
use strata_consensus_logic::{csm::ctl::CsmController, l1_handler::bitcoin_data_handler_task};
use strata_db::traits::{Database, L1DataProvider};
use strata_primitives::params::Params;
use strata_status::StatusTx;
use strata_tasks::TaskExecutor;
use strata_tx_parser::messages::L1Event;
use tokio::sync::mpsc;

use crate::config::Config;

pub fn start_reader_tasks<D>(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl Reader + Send + Sync + 'static>,
    db: Arc<D>,
    csm_ctl: Arc<CsmController>,
    status_rx: Arc<StatusTx>,
) -> anyhow::Result<()>
where
    D: Database + Send + Sync + 'static,
{
    let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

    // TODO switch to checking the L1 tip in the consensus/client state
    let l1prov = db.l1_provider().clone();
    let target_next_block = l1prov
        .get_chain_tip()?
        .map(|i| i + 1)
        .unwrap_or(params.rollup().horizon_l1_height);

    let reader_config = Arc::new(config.get_reader_config(params.clone()));
    let chprov = db.chain_state_provider().clone();

    executor.spawn_critical_async(
        "bitcoin_data_reader_task",
        bitcoin_data_reader_task::<D>(
            rpc_client,
            ev_tx,
            target_next_block,
            reader_config,
            status_rx.clone(),
            chprov,
        ),
    );

    let l1db = db.l1_store().clone();
    let _sedb = db.sync_event_store().clone();

    executor.spawn_critical("bitcoin_data_handler_task", move |_| {
        bitcoin_data_handler_task::<D>(l1db, csm_ctl, ev_rx, params)
    });
    Ok(())
}
