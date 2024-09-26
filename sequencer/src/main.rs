use std::{fs, sync::Arc, time::Duration};

use alpen_express_btcio::broadcaster::L1BroadcastHandle;
use alpen_express_common::logging;
use alpen_express_consensus_logic::{
    self, checkpoint::CheckpointHandle, genesis, state_tracker, sync_manager::SyncManager,
};
use alpen_express_db::traits::Database;
use alpen_express_primitives::params::{Params, SyncParams};
use alpen_express_rocksdb::DbOpsConfig;
use alpen_express_rpc_api::AlpenApiServer;
use alpen_express_rpc_types::L1Status;
use alpen_express_state::csm_status::CsmStatus;
use alpen_express_status::{create_status_channel, StatusRx, StatusTx};
use anyhow::Context;
use config::{ClientMode, Config};
use express_bridge_relay::relayer::RelayerHandle;
use express_storage::{managers::checkpoint::CheckpointDbManager, L2BlockManager};
use express_sync::{self, L2SyncContext, RpcSyncPeer};
use express_tasks::{ShutdownSignal, TaskManager};
use helpers::{
    create_bitcoin_rpc, get_config, init_broadcast_handle, init_core_dbs, init_sequencer,
    init_tasks, initialize_sequencer_database, load_rollup_params_or_default,
};
use jsonrpsee::Methods;
use rockbound::rocksdb;
use rpc_client::sync_client;
use tokio::sync::oneshot;
use tracing::*;

use crate::args::Args;

mod args;
mod config;
mod extractor;
mod helpers;
mod l1_reader;
mod rpc_client;
mod rpc_server;

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("FATAL ERROR: {e}");
        // eprintln!("trace:\n{e:?}");
        // TODO: error code ?

        return Err(e);
    }

    Ok(())
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    logging::init();

    let config = get_config(args.clone())?;

    // Set up block params.
    let params: Arc<_> = Params {
        // FIXME this .expect breaks printing errors
        rollup: load_rollup_params_or_default(&args.rollup_params).expect("rollup params"),
        run: SyncParams {
            // FIXME these shouldn't be configurable here
            l1_follow_distance: config.sync.l1_follow_distance,
            client_checkpoint_interval: config.sync.client_checkpoint_interval,
            l2_blocks_fetch_limit: config.client.l2_blocks_fetch_limit,
        },
    }
    .into();

    let mut methods = jsonrpsee::Methods::new();

    // Open and initialize the database.
    let rbdb = open_rocksdb_database(&config)?;

    let db_ops = DbOpsConfig {
        retry_count: config.client.db_retry_count,
    };

    let (database, broadcast_database) = init_core_dbs(rbdb.clone(), db_ops);

    // Start runtime for async IO tasks.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("express-rt")
        .build()
        .expect("init: build rt");

    // Init thread pool for batch jobs.
    // TODO switch to num_cpus
    let pool = threadpool::ThreadPool::with_name("express-pool".to_owned(), 8);

    let task_manager = TaskManager::new(rt.handle().clone());
    let executor = task_manager.executor();

    // Set up bridge messaging stuff.
    // TODO move all of this into relayer task init
    let bridge_msg_db = Arc::new(alpen_express_rocksdb::BridgeMsgDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let bridge_msg_ctx = express_storage::ops::bridge_relay::Context::new(bridge_msg_db);
    let bridge_msg_ops = Arc::new(bridge_msg_ctx.into_ops(pool.clone()));

    let checkpoint_manager: Arc<_> =
        CheckpointDbManager::new(pool.clone(), database.clone()).into();
    let checkpoint_handle: Arc<_> = CheckpointHandle::new(checkpoint_manager.clone()).into();
    let btc_rpc = create_bitcoin_rpc(&config)?;

    let broadcast_handle =
        init_broadcast_handle(broadcast_database, pool.clone(), &executor, btc_rpc.clone());

    let (stop_tx, stop_rx) = oneshot::channel();

    let mgr_ctx = init_tasks(
        pool.clone(),
        database.clone(),
        params.clone(),
        &config,
        &rt,
        &executor,
        checkpoint_manager,
    )?;

    // Start relayer task.
    // TODO cleanup, this is ugly
    let start_relayer_fut = express_bridge_relay::relayer::start_bridge_relayer_task(
        bridge_msg_ops,
        mgr_ctx.status_rx.clone(),
        config.relayer,
        &executor,
    );

    // FIXME this init is screwed up because of the order we start things
    let relayer_handle = rt.block_on(start_relayer_fut)?;

    // If we're a sequencer, start the sequencer db and duties task.
    if let ClientMode::Sequencer(sequencer_config) = &config.client.client_mode {
        let seq_db = initialize_sequencer_database(rbdb.clone(), db_ops);
        init_sequencer(
            sequencer_config,
            &config,
            btc_rpc.clone(),
            &task_manager,
            seq_db,
            &mgr_ctx,
            checkpoint_handle.clone(),
            broadcast_handle.clone(),
            stop_tx,
            &mut methods,
        )?;
    };

    // Start the L1 tasks to get that going.
    let csm_ctl = mgr_ctx.sync_manager.get_csm_ctl();
    l1_reader::start_reader_tasks(
        &executor,
        mgr_ctx.sync_manager.get_params(),
        &config,
        btc_rpc.clone(),
        database.clone(),
        csm_ctl,
        mgr_ctx.status_tx.clone(),
    )?;

    if let ClientMode::FullNode(fullnode_config) = &config.client.client_mode {
        let sequencer_rpc = &fullnode_config.sequencer_rpc;
        info!(?sequencer_rpc, "initing fullnode task");

        let rpc_client = rt.block_on(sync_client(sequencer_rpc));
        let sync_peer = RpcSyncPeer::new(rpc_client, 10);
        let l2_sync_context = L2SyncContext::new(
            sync_peer,
            mgr_ctx.l2block_manager.clone(),
            mgr_ctx.sync_manager.clone(),
        );
        // NOTE: this might block for some time during first run with empty db until genesis block
        // is generated
        let mut l2_sync_state =
            express_sync::block_until_csm_ready_and_init_sync_state(&l2_sync_context)?;

        executor.spawn_critical_async("l2-sync-manager", async move {
            express_sync::sync_worker(&mut l2_sync_state, &l2_sync_context)
                .await
                .map_err(Into::into)
        });
    }

    let shutdown_signal = task_manager.shutdown_signal();
    let db_cloned = database.clone();

    let l2block_man = mgr_ctx.l2block_manager.clone();
    executor.spawn_critical_async(
        "main-rpc",
        start_rpc(
            shutdown_signal,
            config,
            mgr_ctx.sync_manager,
            db_cloned,
            mgr_ctx.status_rx,
            broadcast_handle,
            l2block_man,
            checkpoint_handle,
            relayer_handle,
            stop_rx,
            methods,
        ),
    );

    task_manager.start_signal_listeners();
    task_manager.monitor(Some(Duration::from_secs(5)))?;

    info!("exiting");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_rpc<D>(
    shutdown_signal: ShutdownSignal,
    config: Config,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    status_rx: Arc<StatusRx>,
    bcast_handle: Arc<L1BroadcastHandle>,
    l2_block_manager: Arc<L2BlockManager>,
    checkpt_handle: Arc<CheckpointHandle>,
    relayer_handle: Arc<RelayerHandle>,
    stop_rx: oneshot::Receiver<()>,
    mut methods: Methods,
) -> anyhow::Result<()>
where
    D: Database + Send + Sync + 'static,
{
    // Init RPC impls.
    let alp_rpc = rpc_server::AlpenRpcImpl::new(
        status_rx.clone(),
        database.clone(),
        sync_man.clone(),
        bcast_handle.clone(),
        l2_block_manager.clone(),
        checkpt_handle.clone(),
        relayer_handle,
    );

    methods.merge(alp_rpc.into_rpc())?;

    let rpc_host = config.client.rpc_host;
    let rpc_port = config.client.rpc_port;

    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(format!("{rpc_host}:{rpc_port}"))
        .await
        .expect("init: build rpc server");

    let rpc_handle = rpc_server.start(methods);

    // start a Btcio event handler
    info!("started RPC server");

    // Wait for a stop signal.
    let _ = stop_rx.await;

    // Send shutdown to all tasks
    shutdown_signal.send();

    // Now start shutdown tasks.
    if rpc_handle.stop().is_err() {
        warn!("RPC server already stopped");
    }

    // wait for rpc to stop
    rpc_handle.stopped().await;

    Ok(())
}

fn open_rocksdb_database(
    config: &Config,
) -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let mut database_dir = config.client.datadir.clone();
    database_dir.push("rocksdb");

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = alpen_express_rocksdb::ROCKSDB_NAME;
    let cfs = alpen_express_rocksdb::STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = rockbound::OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )
    .context("opening database")?;

    Ok(Arc::new(rbdb))
}

// initializes the status bundle that we can pass around cheaply for as name suggests status/metrics
fn start_status<D>(
    database: Arc<D>,
    params: Arc<Params>,
) -> anyhow::Result<(Arc<StatusTx>, Arc<StatusRx>)>
where
    <D as Database>::ClientStateProvider: Send + Sync + 'static,
    D: Database + Send + Sync + 'static,
{
    // Check if we have to do genesis.
    if genesis::check_needs_client_init(database.as_ref())? {
        info!("need to init client state!");
        genesis::init_client_state(&params, database.as_ref())?;
    }
    // init client state
    let cs_prov = database.client_state_provider().as_ref();
    let (cur_state_idx, cur_state) = state_tracker::reconstruct_cur_state(cs_prov)?;

    // init the CsmStatus
    let mut status = CsmStatus::default();
    status.set_last_sync_ev_idx(cur_state_idx);
    status.update_from_client_state(&cur_state);

    Ok(create_status_channel(
        status,
        cur_state,
        L1Status::default(),
    ))
}
