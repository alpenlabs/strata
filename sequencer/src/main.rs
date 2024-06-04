use std::process;

use thiserror::Error;
use tokio::sync::oneshot;
use tracing::*;

use alpen_vertex_common::logging;
use alpen_vertex_rpc_api::AlpenApiServer;

use crate::args::Args;

mod args;
mod rpc_server;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("{0}")]
    Other(String),
}

fn main() {
    logging::init();

    let args = argh::from_env();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex")
        .build()
        .expect("init: build rt");

    if let Err(e) = rt.block_on(main_task(args)) {
        error!(err = %e, "main task exited");
        process::exit(0);
    }

    info!("exiting");
}

async fn main_task(args: Args) -> Result<(), InitError> {
    let (stop_tx, stop_rx) = oneshot::channel();

    // Init RPC methods.
    let alp_rpc = rpc_server::AlpenRpcImpl::new(stop_tx);
    let methods = alp_rpc.into_rpc();

    let rpc_port = args.rpc_port; // TODO make configurable
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
