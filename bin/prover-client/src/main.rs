//! Prover client.

use std::{sync::Arc, time::Duration};

use alpen_express_btcio::rpc::BitcoinClient;
use alpen_express_common::logging;
use alpen_express_rpc_types::RpcCheckpointInfo;
use anyhow::Context;
use args::Args;
use config::{
    BTC_DISPATCH_INTERVAL, BTC_START_BLOCK, L1_BATCH_DISPATCH_INTERVAL, L2_DISPATCH_INTERVAL,
    L2_START_BLOCK, PROVER_MANAGER_INTERVAL,
};
use dispatcher::TaskDispatcher;
use express_sp1_adapter::SP1Host;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use manager::ProverManager;
use proving_ops::{
    btc_ops::BtcOperations, checkpoint_ops::CheckpointOperations, cl_ops::ClOperations,
    el_ops::ElOperations, l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations,
};
use rpc_server::{ProverClientRpc, RpcContext};
use task::TaskTracker;
use tokio::time::sleep;
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

    let checkpoint_info = wait_for_first_checkpoint(&cl_client)
        .await
        .expect("failed to fetch checkpoint");

    let task_tracker = Arc::new(TaskTracker::new());

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

    let cl_ops = ClOperations::new(cl_client.clone(), Arc::new(el_dispatcher.clone()));
    let mut cl_dispatcher = TaskDispatcher::new(
        cl_ops,
        task_tracker.clone(),
        L2_START_BLOCK,
        Duration::from_secs(L2_DISPATCH_INTERVAL),
    );

    // Initialize l1_batch_ops and its dispatcher
    let l1_batch_ops = L1BatchOperations::new(Arc::new(btc_dispatcher.clone()));
    let l1_batch_dispatcher = TaskDispatcher::new(
        l1_batch_ops,
        task_tracker.clone(),
        checkpoint_info.l1_range,
        Duration::from_secs(L1_BATCH_DISPATCH_INTERVAL),
    );

    let l2_batch_ops = L2BatchOperations::new(Arc::new(cl_dispatcher.clone()).clone());
    let l2_batch_dispatcher = TaskDispatcher::new(
        l2_batch_ops,
        task_tracker.clone(),
        checkpoint_info.l2_range,
        Duration::from_secs(L2_DISPATCH_INTERVAL),
    );

    let checkpoint_ops = CheckpointOperations::new(
        Arc::new(l1_batch_dispatcher.clone()),
        Arc::new(l2_batch_dispatcher.clone()),
    );
    let checkpoint_dispatcher = TaskDispatcher::new(
        checkpoint_ops,
        task_tracker.clone(),
        checkpoint_info,
        Duration::from_secs(L2_DISPATCH_INTERVAL),
    );

    let rpc_context = RpcContext::new(
        btc_dispatcher.clone(),
        el_dispatcher.clone(),
        cl_dispatcher.clone(),
        l1_batch_dispatcher.clone(),
        l2_batch_dispatcher.clone(),
        checkpoint_dispatcher.clone(),
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

async fn wait_for_first_checkpoint(
    cl_client: &HttpClient,
) -> Result<RpcCheckpointInfo, anyhow::Error> {
    let checkpoint_idx: u64 = 1;

    loop {
        let checkpoint_info: Option<RpcCheckpointInfo> = cl_client
            .request("alp_getCheckpointInfo", rpc_params!([checkpoint_idx]))
            .await
            .context("Failed to get the checkpoint info")?;

        // Check if we have a checkpoint info
        if let Some(info) = checkpoint_info {
            return Ok(info);
        }

        // Wait before querying again
        sleep(Duration::from_secs(PROVER_MANAGER_INTERVAL)).await;
    }
}
