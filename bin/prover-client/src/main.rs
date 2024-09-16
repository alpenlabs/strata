//! Prover client.

use std::sync::Arc;

use alpen_express_common::logging;
use args::Args;
use express_risc0_adapter::RiscZeroHost;
use jsonrpsee::http_client::HttpClientBuilder;
use manager::ProverManager;
use rpc_server::{ProverClientRpc, RpcContext};
use task_dispatcher::ELBlockProvingTaskScheduler;
use task_tracker::TaskTracker;
use tracing::info;

mod args;
pub(crate) mod config;
pub(crate) mod manager;
pub(crate) mod primitives;
pub(crate) mod prover;
pub(crate) mod rpc_server;
pub(crate) mod task_dispatcher;
pub(crate) mod task_tracker;

#[tokio::main]
async fn main() {
    logging::init();
    info!("running prover client in dev mode");

    let args: Args = argh::from_env();
    let task_tracker = Arc::new(TaskTracker::new());

    let el_rpc_client = HttpClientBuilder::default()
        .build(args.get_reth_rpc_url())
        .expect("failed to connect to the el client");

    let el_proving_task_scheduler =
        ELBlockProvingTaskScheduler::new(el_rpc_client, task_tracker.clone());
    let rpc_context = RpcContext::new(el_proving_task_scheduler.clone());
    let prover_manager: ProverManager<RiscZeroHost> = ProverManager::new(task_tracker);

    // run prover manager in background
    tokio::spawn(async move {
        prover_manager.run().await;
    });

    // run el proving task dispatcher
    tokio::spawn(async move {
        el_proving_task_scheduler
            .clone()
            .listen_for_new_blocks()
            .await
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
