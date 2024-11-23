//! Prover client.

use std::sync::Arc;

use args::Args;
use ckp_runner::start_checkpoints_task;
use dispatcher::TaskDispatcher;
use jsonrpsee::http_client::HttpClientBuilder;
use manager::ProverManager;
use proving_ops::{
    btc_ops::BtcOperations, checkpoint_ops::CheckpointOperations, cl_ops::ClOperations,
    el_ops::ElOperations, l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations,
};
use rpc_server::{ProverClientRpc, RpcContext};
use strata_btcio::rpc::BitcoinClient;
use strata_common::logging;
use task::TaskTracker;
use tracing::{debug, info};

mod args;
mod ckp_runner;
mod config;
mod db;
mod dispatcher;
mod errors;
mod hosts;
mod manager;
mod primitives;
mod prove;
mod proving_ops;
mod rpc_server;
mod state;
mod task;
mod task2;

#[tokio::main]
async fn main() {
    logging::init(logging::LoggerConfig::with_base_name(
        "strata-prover-client",
    ));
    info!("Running strata prover client in dev mode");

    let args: Args = argh::from_env();
    debug!("Running prover client with args {:?}", args);

    let el_client = HttpClientBuilder::default()
        .build(args.get_reth_rpc_url())
        .expect("failed to connect to the el client");

    let cl_client = HttpClientBuilder::default()
        .build(args.get_sequencer_rpc_url())
        .expect("failed to connect to the el client");

    let btc_client = Arc::new(
        BitcoinClient::new(
            args.get_btc_rpc_url(),
            args.bitcoind_user.clone(),
            args.bitcoind_password.clone(),
        )
        .expect("failed to connect to the btc client"),
    );

    let task_tracker = Arc::new(TaskTracker::new());

    // Create L1 operations
    let btc_ops = BtcOperations::new(btc_client.clone());
    let btc_dispatcher = TaskDispatcher::new(btc_ops, task_tracker.clone());

    // Create EL  operations
    let el_ops = ElOperations::new(el_client.clone());
    let el_dispatcher = TaskDispatcher::new(el_ops, task_tracker.clone());

    let cl_ops = ClOperations::new(cl_client.clone(), Arc::new(el_dispatcher.clone()));
    let cl_dispatcher = TaskDispatcher::new(cl_ops, task_tracker.clone());

    let l1_batch_ops = L1BatchOperations::new(Arc::new(btc_dispatcher.clone()), btc_client.clone());
    let l1_batch_dispatcher = TaskDispatcher::new(l1_batch_ops, task_tracker.clone());

    let l2_batch_ops = L2BatchOperations::new(Arc::new(cl_dispatcher.clone()).clone());
    let l2_batch_dispatcher = TaskDispatcher::new(l2_batch_ops, task_tracker.clone());

    let checkpoint_ops = CheckpointOperations::new(
        cl_client.clone(),
        Arc::new(l1_batch_dispatcher.clone()),
        Arc::new(l2_batch_dispatcher.clone()),
    );

    let checkpoint_dispatcher = TaskDispatcher::new(checkpoint_ops, task_tracker.clone());

    let rpc_context = RpcContext::new(
        btc_dispatcher.clone(),
        el_dispatcher.clone(),
        cl_dispatcher.clone(),
        l1_batch_dispatcher.clone(),
        l2_batch_dispatcher.clone(),
        checkpoint_dispatcher.clone(),
    );

    let prover_manager: ProverManager = ProverManager::new(task_tracker.clone());

    // run prover manager in background
    tokio::spawn(async move { prover_manager.run().await });

    // run checkpoint runner
    tokio::spawn(async move {
        start_checkpoints_task(
            cl_client.clone(),
            checkpoint_dispatcher.clone(),
            task_tracker.clone(),
        )
        .await
    });

    // Run prover manager in dev mode or runner mode
    if args.enable_dev_rpcs {
        // Run the rpc server on dev mode only
        let rpc_url = args.get_dev_rpc_url();
        run_rpc_server(rpc_context, rpc_url, args.enable_dev_rpcs)
            .await
            .expect("prover client rpc")
    }
}

async fn run_rpc_server(
    rpc_context: RpcContext,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()> {
    let rpc_impl = ProverClientRpc::new(rpc_context);
    rpc_server::start(&rpc_impl, rpc_url, enable_dev_rpc).await?;
    anyhow::Ok(())
}
