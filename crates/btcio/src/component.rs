use bitcoin::Network;
use strata_component::{
    component::{ClientComponent, ComponentBuilder},
    context::{BuildContext, ComponentHandle, RunContext},
    CsmHandle,
};
use tracing::*;

use crate::rpc::BitcoinClient;

pub struct L1Reader {
    client: BitcoinClient,
}

pub struct L1ReaderBuilder;

impl ComponentBuilder for L1ReaderBuilder {
    type Output = L1Reader;

    fn build(&self, buildctx: &BuildContext) -> L1Reader {
        // Set up Bitcoin client RPC.
        let bitcoind_url = format!("http://{}", buildctx.config.bitcoind_rpc.rpc_url);
        let client = BitcoinClient::new(
            bitcoind_url,
            buildctx.config.bitcoind_rpc.rpc_user.clone(),
            buildctx.config.bitcoind_rpc.rpc_password.clone(),
        )
        .expect("Could not build L1Reader Component");

        // TODO remove this
        if buildctx.config.bitcoind_rpc.network != Network::Regtest {
            warn!("network not set to regtest, ignoring");
        }
        L1Reader { client }
    }
}

// NOTE: not sure if this L1Reader should be defined in btcio. in general whether a component should
// be defined in the respective crates instead of defining centrally like this
impl ClientComponent for L1Reader {
    fn validate(&self) {}

    fn run(&self, runctx: RunContext) -> ComponentHandle {
        // let (ev_tx, ev_rx) = mpsc::channel::<L1Event>(100); // TODO: think about the buffer size

        // // TODO switch to checking the L1 tip in the consensus/client state
        // let l1_db = db.l1_db().clone();
        // let horz_height = runctx.params.rollup().horizon_l1_height;
        // let target_next_block = l1_db.get_chain_tip()?.map(|i| i + 1).unwrap_or(horz_height);
        // assert!(target_next_block >= horz_height);

        // let reader_config = Arc::new(ReaderConfig::from_config_and_params(
        //     runctx.config.clone(),
        //     runctx.params.clone(),
        // ));

        // runctx.task_manager.executor().spawn_critical_async(
        //     "bitcoin_data_reader_task",
        //     bitcoin_data_reader_task(
        //         self.client,
        //         ev_tx,
        //         target_next_block,
        //         reader_config,
        //         runctx.status_channel,
        //     ),
        // );

        // let l1db = db.l1_db().clone();
        // let _sedb = db.sync_event_db().clone();

        // runctx
        //     .task_manager
        //     .executor()
        //     .spawn_critical("bitcoin_data_handler_task", move |_| {
        //         bitcoin_data_handler_task::<D>(l1db, csm_ctl, ev_rx, runctx.params)
        //     });
        ComponentHandle
    }
}
