use std::sync::Arc;

use strata_btcio::{reader::query::bitcoin_data_reader_task, rpc::traits::ReaderRpc};
use strata_config::Config;
use strata_consensus_logic::{csm::ctl::CsmController, l1_handler::bitcoin_data_handler_task};
use strata_db::traits::{Database, L1Database};
use strata_l1tx::messages::L1Event;
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_tasks::TaskExecutor;
use tokio::sync::mpsc;

pub fn start_reader_tasks<D>(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl ReaderRpc + Send + Sync + 'static>,
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

    executor.spawn_critical_async(
        "bitcoin_data_reader_task",
        bitcoin_data_reader_task(
            rpc_client,
            ev_tx,
            target_next_block,
            Arc::new(config.btcio.reader.clone()),
            params.clone(),
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
