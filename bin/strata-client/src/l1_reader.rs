use std::sync::Arc;

use strata_btcio::{reader::query::bitcoin_data_reader_task, rpc::traits::Reader};
use strata_consensus_logic::{csm::ctl::CsmController, l1_handler::bitcoin_data_handler_task};
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_storage::managers::l1::L1BlockManager;
use strata_tasks::TaskExecutor;
use strata_tx_parser::messages::L1Event;
use tokio::sync::mpsc;

use crate::config::Config;

pub fn start_reader_tasks(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl Reader + Send + Sync + 'static>,
    l1_manager: Arc<L1BlockManager>,
    csm_ctl: Arc<CsmController>,
    status_channel: StatusChannel,
) -> anyhow::Result<()> {
    let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

    let horz_height = params.rollup().horizon_l1_height;
    let target_next_block = status_channel.l1_view().next_expected_block();
    assert!(target_next_block >= horz_height);

    let reader_config = Arc::new(config.get_reader_config(params.clone()));

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

    executor.spawn_critical("bitcoin_data_handler_task", move |_| {
        bitcoin_data_handler_task(l1_manager, csm_ctl, ev_rx, params)
    });
    Ok(())
}
