use std::io;
use std::process;
use std::sync::Arc;

use anyhow::Context;
use reth_rpc_types::ParseBlockHashOrNumberError;
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
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Other(String),
}

fn main() {
    let args: Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("FATAL ERROR: {e}");
    }
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    logging::init();

    // Open the database.
    let rbdb = open_rocksdb_database(&args)?;

    // Initialize stubs.
    let sync_ev_db = alpen_vertex_db::SyncEventDb::new(rbdb.clone());
    let cs_db = alpen_vertex_db::ConsensusStateDb::new(rbdb.clone());
    let l1_db = alpen_vertex_db::L1Db::new(rbdb.clone());

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex")
        .build()
        .expect("init: build rt");

    if let Err(e) = rt.block_on(main_task(args)) {
        error!(err = %e, "main task exited");
        process::exit(0); // special case exit once we've gotten to this point
    }

    info!("exiting");
    Ok(())
}

async fn main_task(args: Args) -> anyhow::Result<()> {
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

fn open_rocksdb_database(args: &Args) -> anyhow::Result<Arc<rockbound::DB>> {
    let mut database_dir = args.datadir.clone();
    database_dir.push("rocksdb");

    let dbname = alpen_vertex_db::ROCKSDB_NAME;
    let cfs = alpen_vertex_db::STORE_COLUMN_FAMILIES;
    let opts = rocksdb::Options::default();

    let rbdb = rockbound::DB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )
    .context("opening database")?;

    Ok(Arc::new(rbdb))
}
