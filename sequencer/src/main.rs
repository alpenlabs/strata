use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::thread;
use std::time;

use anyhow::Context;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::*;

use alpen_vertex_btcio::rpc::traits::L1Client;
use alpen_vertex_common::logging;
use alpen_vertex_consensus_logic::ctl::CsmController;
use alpen_vertex_consensus_logic::duties::{DutyBatch, Identity};
use alpen_vertex_consensus_logic::duty_executor::{self, IdentityData, IdentityKey};
use alpen_vertex_consensus_logic::message::{ChainTipMessage, ConsensusUpdateNotif, CsmMessage};
use alpen_vertex_consensus_logic::sync_manager;
use alpen_vertex_consensus_logic::sync_manager::SyncManager;
use alpen_vertex_consensus_logic::{chain_tip, unfinalized_tracker, worker};
use alpen_vertex_db::database::CommonDatabase;
use alpen_vertex_db::stubs::l2::StubL2Db;
use alpen_vertex_db::traits::Database;
use alpen_vertex_db::{ConsensusStateDb, L1Db, SyncEventDb};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::{block_credential, params::*};
use alpen_vertex_rpc_api::AlpenApiServer;
use alpen_vertex_state::consensus::ConsensusState;
use alpen_vertex_state::operation;

use crate::args::Args;

mod args;
mod config;
mod l1_reader;
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
        eprintln!("trace:\n{e:?}");
    }
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    logging::init();

    // Open the database.
    let rbdb = open_rocksdb_database(&args)?;

    // Set up block params.
    let params = Params {
        rollup: RollupParams {
            block_time: 1000,
            cred_rule: block_credential::CredRule::Unchecked,
            l1_start_block_height: 4,
        },
        run: RunParams {
            l1_follow_distance: 6,
        },
    };
    let params = Arc::new(params);

    // Start runtime for async IO tasks.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex-rt")
        .build()
        .expect("init: build rt");

    // Init thread pool for batch jobs.
    // TODO switch to num_cpus maybe?  we don't want to compete with tokio though
    let pool = Arc::new(threadpool::ThreadPool::with_name(
        "vertex-pool".to_owned(),
        8,
    ));

    // Initialize databases.
    let l1_db = Arc::new(alpen_vertex_db::L1Db::new(rbdb.clone()));
    let l2_db = Arc::new(alpen_vertex_db::stubs::l2::StubL2Db::new()); // FIXME stub
    let sync_ev_db = Arc::new(alpen_vertex_db::SyncEventDb::new(rbdb.clone()));
    let cs_db = Arc::new(alpen_vertex_db::ConsensusStateDb::new(rbdb.clone()));
    let database = Arc::new(alpen_vertex_db::database::CommonDatabase::new(
        l1_db, l2_db, sync_ev_db, cs_db,
    ));

    // Set up Bitcoin client RPC.
    let bitcoind_url = format!("http://{}", args.bitcoind_host);
    let btc_rpc = alpen_vertex_btcio::rpc::BitcoinClient::new(
        bitcoind_url,
        args.bitcoind_user.clone(),
        args.bitcoind_password.clone(),
        bitcoin::Network::Regtest,
    );

    // TODO remove this
    if args.network != "regtest" {
        warn!(network = %args.network, "network not set to regtest, ignoring");
    }

    // Create dataflow channels.
    let (cout_tx, cout_rx) = mpsc::channel::<operation::ConsensusOutput>(64);
    let (cur_state_tx, cur_state_rx) = watch::channel::<Option<ConsensusState>>(None);
    // TODO connect up these other channels

    // Init engine controller.
    let eng_ctl = alpen_vertex_evmctl::stub::StubController::new(time::Duration::from_millis(100));
    let eng_ctl = Arc::new(eng_ctl);

    // Start the sync manager.
    let sync_man = sync_manager::start_sync_tasks(
        database.clone(),
        eng_ctl.clone(),
        pool.clone(),
        params.clone(),
    )?;
    let sync_man = Arc::new(sync_man);

    // If the sequencer key is set, start the sequencer duties task.
    if let Some(seqkey_path) = &args.sequencer_key {
        info!(?seqkey_path, "initing sequencer duties task");
        let idata = load_seqkey(seqkey_path)?;

        // Set up channel and clone some things.
        let sm = sync_man.clone();
        let cu_rx = sync_man.create_cstate_subscription();
        let (duties_tx, duties_rx) = broadcast::channel::<DutyBatch>(8);
        let db = database.clone();
        let db2 = database.clone();
        let eng_ctl_de = eng_ctl.clone();
        let pool = pool.clone();

        // Spawn the two tasks.
        thread::spawn(move || {
            // FIXME figure out why this can't infer the type, it's like *right there*
            duty_executor::duty_tracker_task::<_, alpen_vertex_evmctl::stub::StubController>(
                cu_rx,
                duties_tx,
                idata.ident,
                db,
            )
        });
        thread::spawn(move || {
            duty_executor::duty_dispatch_task(duties_rx, idata.key, sm, db2, eng_ctl_de, pool)
        });
    }

    let main_fut = main_task(args, sync_man, btc_rpc, database.clone());
    if let Err(e) = rt.block_on(main_fut) {
        error!(err = %e, "main task exited");
        process::exit(0); // special case exit once we've gotten to this point
    }

    info!("exiting");
    Ok(())
}

async fn main_task<D: Database>(
    args: Args,
    sync_man: Arc<SyncManager>,
    l1_rpc_client: impl L1Client,
    database: Arc<D>,
) -> anyhow::Result<()>
where
    // TODO how are these not redundant trait bounds???
    <D as alpen_vertex_db::traits::Database>::SeStore: Send + Sync + 'static,
    <D as alpen_vertex_db::traits::Database>::L1Store: Send + Sync + 'static,
{
    l1_reader::start_reader_tasks(sync_man.params(), l1_rpc_client, database.clone()).await?;

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

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = alpen_vertex_db::ROCKSDB_NAME;
    let cfs = alpen_vertex_db::STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = rockbound::DB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )
    .context("opening database")?;

    Ok(Arc::new(rbdb))
}

fn load_seqkey(path: &PathBuf) -> anyhow::Result<IdentityData> {
    let Ok(raw_key) = <[u8; 32]>::try_from(fs::read(path)?) else {
        error!("malformed seqkey");
        anyhow::bail!("malformed seqkey");
    };

    let key = Buf32::from(raw_key);

    // FIXME all this needs to be changed to use actual cryptographic keys
    let ik = IdentityKey::Sequencer(key);
    let ident = Identity::Sequencer(key);
    let idata = IdentityData::new(ident, ik);

    Ok(idata)
}
