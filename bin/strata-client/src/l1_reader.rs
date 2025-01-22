use std::sync::Arc;

use strata_btcio::{reader::query::bitcoin_data_reader_task, rpc::traits::ReaderRpc};
use strata_config::Config;
use strata_consensus_logic::csm::ctl::CsmController;
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_storage::{L1BlockManager, NodeStorage};
use strata_tasks::TaskExecutor;

pub fn start_reader_tasks(
    executor: &TaskExecutor,
    params: Arc<Params>,
    config: &Config,
    rpc_client: Arc<impl ReaderRpc + Send + Sync + 'static>,
    storage: &NodeStorage,
    csm_ctl: Arc<CsmController>,
    status_channel: StatusChannel,
) -> anyhow::Result<()> {
    // TODO switch to checking the L1 tip in the consensus/client state
    let horz_height = params.rollup().horizon_l1_height;
    let target_next_block = l1_manager
        .get_chain_tip()?
        .map(|i| i + 1)
        .unwrap_or(horz_height);
    assert!(target_next_block >= horz_height);

    let l1man = storage.l1().clone();
    let csm = csm_ctl.clone();
    executor.spawn_critical_async(
        "bitcoin_data_reader_task",
        bitcoin_data_reader_task(
            rpc_client,
            l1man,
            target_next_block,
            Arc::new(config.btcio.reader.clone()),
            params.clone(),
            status_channel,
            move |ev| csm.clone().submit_event(ev),
        ),
    );
    Ok(())
}
