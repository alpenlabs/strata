//! Prover client.

use std::sync::Arc;

use alpen_express_common::logging;
use args::Args;
use express_risc0_adapter::RiscZeroHost;
use manager::ProverManager;
use rpc_server::{ProverClientRpc, RpcContext};
use task_tracker::TaskTracker;
use tracing::info;

mod args;
pub(crate) mod config;
pub(crate) mod manager;
pub(crate) mod primitives;
pub(crate) mod proving;
pub(crate) mod rpc_server;
pub(crate) mod task_tracker;

#[tokio::main]
async fn main() {
    logging::init();
    info!("running prover client in dev mode");

    let args: Args = argh::from_env();
    let task_tracker = Arc::new(TaskTracker::new());
    let rpc_context = RpcContext::new(
        task_tracker.clone(),
        args.get_sequencer_rpc_url(),
        args.get_reth_rpc_url(),
    );

    let prover_manager: ProverManager<RiscZeroHost> = ProverManager::new(task_tracker.clone());

    // run prover manager in background
    tokio::spawn(async move {
        prover_manager.run().await;
    });

    // run rpc server
    let rpc_url = args.get_rpc_url();
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
