use std::process;

use reth_rpc_api::EthApiServer;
use thiserror::Error;
use tokio::sync::oneshot;
use tracing::*;

use alpen_vertex_common::logging;
use alpen_vertex_rpc_api::AlpenApiServer;

mod rpc_server;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("{0}")]
    Other(String),
}

fn main() {
    logging::init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex")
        .build()
        .expect("init: build rt");

    if let Err(e) = rt.block_on(main_task()) {
        error!(err = %e, "main task exited");
        process::exit(0);
    }

    info!("exiting");
}

async fn main_task() -> Result<(), InitError> {
    let (stop_tx, stop_rx) = oneshot::channel();

    // Init RPC methods.
    let alp_rpc = rpc_server::AlpenRpcImpl::new(stop_tx);
    let eth_rpc = rpc_server::EthRpcImpl::new();
    let mut methods = alp_rpc.into_rpc();
    methods
        .merge(eth_rpc.into_rpc())
        .expect("init: add eth methods");

    let rpc_port = 12345; // TODO make configurable
    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(format!("127.0.0.1:{rpc_port}"))
        .await
        .expect("init: build rpc server");

    let rpc_handle = rpc_server.start(methods);
    info!("started RPC server");

    // Wait for a stop signal.
    let _ = stop_rx.await;

    // Now start shutdown tasks.
    if rpc_handle.stop().is_err() {
        warn!("RPC server already stopped");
    }

    Ok(())
}
