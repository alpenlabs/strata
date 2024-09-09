//! Prover client.

use alpen_express_common::logging;
use anyhow::Ok;
use rpc_server::ProverClientRpc;
use tracing::info;

pub(crate) mod constants;
pub(crate) mod rpc_server;

#[tokio::main]
async fn main() {
    logging::init();
    info!("running prover client in dev mode");

    run_inner().await.expect("prover client")
}

async fn run_inner() -> anyhow::Result<()> {
    let rpc_impl = ProverClientRpc::new();
    rpc_server::start(&rpc_impl).await?;
    Ok(())
}
