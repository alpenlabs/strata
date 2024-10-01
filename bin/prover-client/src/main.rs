//! Prover client.

use std::{sync::Arc, time::Duration};

use alpen_express_btcio::rpc::BitcoinClient;
use alpen_express_common::logging;
use args::Args;
use config::{BTC_DISPATCH_INTERVAL, BTC_START_BLOCK, L2_DISPATCH_INTERVAL, L2_START_BLOCK};
use dispatcher::TaskDispatcher;
use express_sp1_adapter::SP1Host;
use jsonrpsee::http_client::HttpClientBuilder;
use manager::ProverManager;
use proving_ops::{btc_ops::BtcOperations, cl_ops::ClOperations, el_ops::ElOperations};
use rpc_server::{ProverClientRpc, RpcContext};
use task::TaskTracker;
use tracing::info;

mod args;
mod config;
mod db;
mod dispatcher;
mod errors;
mod manager;
mod primitives;
mod prover;
mod proving_ops;
mod rpc_server;
mod task;

#[tokio::main]
async fn main() {
    logging::init();
    info!("running alpen express prover client in dev mode");

    let args: Args = argh::from_env();
    let task_tracker = Arc::new(TaskTracker::new());

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
        .unwrap(),
    );

    // Create operations
    let btc_ops = BtcOperations::new(btc_client.clone());
    let el_ops = ElOperations::new(el_client.clone());
    let cl_ops = ClOperations::new(cl_client.clone());

    // Create dispatchers
    let mut btc_dispatcher = TaskDispatcher::new(
        btc_ops,
        task_tracker.clone(),
        BTC_START_BLOCK,
        Duration::from_secs(BTC_DISPATCH_INTERVAL),
    );

    let mut el_dispatcher = TaskDispatcher::new(
        el_ops,
        task_tracker.clone(),
        L2_START_BLOCK,
        Duration::from_secs(L2_DISPATCH_INTERVAL),
    );

    let mut cl_dispatcher = TaskDispatcher::new(
        cl_ops,
        task_tracker.clone(),
        L2_START_BLOCK,
        Duration::from_secs(L2_DISPATCH_INTERVAL),
    );

    let rpc_context = RpcContext::new(
        btc_dispatcher.clone(),
        el_dispatcher.clone(),
        cl_dispatcher.clone(),
    );

    // Run dispatchers in background
    tokio::spawn(async move { btc_dispatcher.start().await });
    tokio::spawn(async move { el_dispatcher.start().await });
    tokio::spawn(async move { cl_dispatcher.start().await });

    let prover_manager: ProverManager<SP1Host> = ProverManager::new(task_tracker);

    // run prover manager in background
    tokio::spawn(async move { prover_manager.run().await });

    // run rpc server
    let rpc_url = args.get_dev_rpc_url();
    run_rpc_server(rpc_context, rpc_url, args.enable_dev_rpcs)
        .await
        .expect("prover client rpc")
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
