use std::sync::Arc;

use strata_btcio::{
    reader::{config::ReaderConfig, query::bitcoin_data_reader_task},
    rpc::traits::Reader,
};
use strata_config::Config;
use strata_consensus_logic::{csm::ctl::CsmController, l1_handler::bitcoin_data_handler_task};
use strata_db::traits::{Database, L1Database};
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_tasks::TaskExecutor;
use strata_tx_parser::messages::L1Event;
use tokio::sync::mpsc;

pub fn start_reader_tasks<D>(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl Reader + Send + Sync + 'static>,
    db: Arc<D>,
    csm_ctl: Arc<CsmController>,
    status_channel: StatusChannel,
) -> anyhow::Result<()>
where
    D: Database + Send + Sync + 'static,
{
    let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

    // TODO switch to checking the L1 tip in the consensus/client state
    let l1_db = db.l1_db().clone();
    let horz_height = params.rollup().horizon_l1_height;
    let target_next_block = l1_db.get_chain_tip()?.map(|i| i + 1).unwrap_or(horz_height);
    assert!(target_next_block >= horz_height);

    let reader_config = Arc::new(ReaderConfig::from_config_and_params(
        config.clone(),
        params.clone(),
    ));

    executor.spawn_critical_async(
        "bitcoin_data_reader_task",
        bitcoin_data_reader_task(
            rpc_client,
            ev_tx,
            target_next_block,
            reader_config,
            status_channel,
        ),
    );

    let l1db = db.l1_db().clone();
    let _sedb = db.sync_event_db().clone();

    executor.spawn_critical("bitcoin_data_handler_task", move |_| {
        bitcoin_data_handler_task::<D>(l1db, csm_ctl, ev_rx, params)
    });
    Ok(())
}
