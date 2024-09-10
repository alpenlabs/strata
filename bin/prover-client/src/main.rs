//! Prover client.

use std::sync::Arc;

use alpen_express_common::logging;
use args::Args;
use models::RpcContext;
use rpc_server::ProverClientRpc;
use task_tracker::TaskTracker;
use tracing::info;
use worker::consumer_worker;

mod args;
pub(crate) mod models;
pub(crate) mod rpc_server;
pub(crate) mod task_tracker;
pub(crate) mod worker;

#[tokio::main]
async fn main() {
    logging::init();
    info!("running prover client in dev mode");
    println!("running prover client in dev mode");

    let args: Args = argh::from_env();
    let task_tracker = Arc::new(TaskTracker::new());
    let rpc_context = RpcContext::new(Arc::clone(&task_tracker), args.get_rpc_url());

    println!("we hav the rpc url {:?}", args.get_rpc_url());

    // Spawn consumer worker
    tokio::spawn(consumer_worker(Arc::clone(&task_tracker)));

    run_rpc_server(rpc_context)
        .await
        .expect("prover client rpc")
}

async fn run_rpc_server(rpc_context: RpcContext) -> anyhow::Result<()> {
    let rpc_url = rpc_context.rpc_url.clone();
    let rpc_impl = ProverClientRpc::new(rpc_context);

    rpc_server::start(&rpc_impl, rpc_url).await?;
    anyhow::Ok(())
}
