//! Prover client.

use std::sync::Arc;

use args::Args;
use config::NUM_PROVER_WORKERS;
// use ckp_runner::start_checkpoints_task;
// use dispatcher::TaskDispatcher;
use jsonrpsee::http_client::HttpClientBuilder;
use manager2::ProverManager;
// use manager::ProverManager;
// use proving_ops::{
//     btc_ops::BtcOperations, checkpoint_ops::CheckpointOperations, cl_ops::ClOperations,
//     el_ops::ElOperations, l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations,
// };
use rpc_server::{ProverClientRpc, ServerRequest};
use strata_btcio::rpc::BitcoinClient;
use strata_common::logging;
use strata_primitives::proof::ProofKey;
use task2::TaskTracker;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

mod args;
// mod ckp_runner;
mod config;
mod db;
// mod dispatcher;
mod errors;
mod handlers;
mod hosts;
// mod manager;
mod manager2;
mod primitives;
// mod prover;
// mod proving_ops;
mod rpc_server;
// mod task;
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

    let btc_client = BitcoinClient::new(
        args.get_btc_rpc_url(),
        args.bitcoind_user.clone(),
        args.bitcoind_password.clone(),
    )
    .expect("failed to connect to the btc client");

    let (task_tx, task_rx) = mpsc::channel::<ServerRequest>(64);

    let manager = ProverManager::init(
        btc_client,
        el_client,
        cl_client,
        task_rx,
        NUM_PROVER_WORKERS,
    );

    // run prover manager in background
    tokio::spawn(async move { manager.process().await });

    // // Run prover manager in dev mode or runner mode
    // if args.enable_dev_rpcs {
    //     // Run the rpc server on dev mode only
    //     let rpc_url = args.get_dev_rpc_url();
    //     run_rpc_server(manager.clone(), rpc_url, args.enable_dev_rpcs)
    //         .await
    //         .expect("prover client rpc")
    // }
}

async fn run_rpc_server(
    task_tx: mpsc::Sender<ServerRequest>,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()> {
    let rpc_impl = ProverClientRpc::new(task_tx);
    rpc_server::start(&rpc_impl, rpc_url, enable_dev_rpc).await?;
    anyhow::Ok(())
}
