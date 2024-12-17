//! Prover client.

use std::sync::Arc;

use args::Args;
use db::open_rocksdb_database;
use handlers::ProofHandler;
use jsonrpsee::http_client::HttpClientBuilder;
use manager::ProverManager;
use rpc_server::ProverClientRpc;
use strata_btcio::rpc::BitcoinClient;
use strata_common::logging;
use strata_rocksdb::{prover::db::ProofDb, DbOpsConfig};
use task::TaskTracker;
use tokio::sync::Mutex;
use tracing::{debug, info};

mod args;
mod db;
mod errors;
mod handlers;
mod hosts;
mod manager;
mod rpc_server;
mod status;
mod task;

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

    let handler = Arc::new(ProofHandler::init(btc_client, el_client, cl_client));
    let task_tracker = Arc::new(Mutex::new(TaskTracker::new()));

    let rbdb = open_rocksdb_database(&args.datadir).expect("failed to open DB");
    let db_ops = DbOpsConfig { retry_count: 3 };
    let db = Arc::new(ProofDb::new(rbdb, db_ops));

    let manager = ProverManager::new(
        task_tracker.clone(),
        handler.clone(),
        db.clone(),
        args.workers,
        args.loop_interval,
    );
    debug!("Initialized Prover Manager");

    // run prover manager in background
    tokio::spawn(async move { manager.process_pending_tasks().await });
    debug!("Spawn process pending tasks");

    // Run prover manager in dev mode or runner mode
    if args.enable_dev_rpcs {
        // Run the rpc server on dev mode only
        let rpc_url = args.get_dev_rpc_url();
        run_rpc_server(
            task_tracker.clone(),
            handler.clone(),
            db.clone(),
            rpc_url,
            args.enable_dev_rpcs,
        )
        .await
        .expect("prover client rpc")
    }
}

async fn run_rpc_server(
    task_tracker: Arc<Mutex<TaskTracker>>,
    handler: Arc<ProofHandler>,
    db: Arc<ProofDb>,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()> {
    let rpc_impl = ProverClientRpc::new(task_tracker, handler, db);
    rpc_server::start(&rpc_impl, rpc_url, enable_dev_rpc).await?;
    anyhow::Ok(())
}
