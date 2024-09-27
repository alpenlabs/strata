use std::{sync::Arc, time::Duration};

use alpen_express_btcio::{
    broadcaster::{spawn_broadcaster_task, L1BroadcastHandle},
    rpc::BitcoinClient,
    writer::{config::WriterConfig, start_inscription_task},
};
use alpen_express_common::logging;
use alpen_express_consensus_logic::{
    checkpoint::CheckpointHandle,
    duty::{types::DutyBatch, worker as duty_worker},
    genesis,
    sync_manager::{self, SyncManager},
};
use alpen_express_db::traits::Database;
use alpen_express_eectl::engine::ExecEngineCtl;
use alpen_express_evmexec::{engine::RpcExecEngineCtl, EngineRpcClient};
use alpen_express_primitives::params::{Params, SyncParams};
use alpen_express_rocksdb::{
    broadcaster::db::BroadcastDatabase, sequencer::db::SequencerDB, DbOpsConfig, RBSeqBlobDb,
};
use alpen_express_rpc_api::{AlpenAdminApiServer, AlpenApiServer, AlpenSequencerApiServer};
use alpen_express_status::{StatusRx, StatusTx};
use config::{ClientMode, Config, SequencerConfig};
use express_bridge_relay::relayer::RelayerHandle;
use express_storage::{managers::checkpoint::CheckpointDbManager, L2BlockManager};
use express_sync::{self, L2SyncContext, RpcSyncPeer};
use express_tasks::{ShutdownSignal, TaskExecutor, TaskManager};
use helpers::{
    create_bitcoin_rpc_client, get_config, init_broadcaster_database, init_core_dbs,
    init_engine_controller, init_sequencer_database, init_status_channel,
    load_rollup_params_or_default, load_seqkey, open_rocksdb_database, CommonDb,
};
use jsonrpsee::Methods;
use rpc_client::sync_client;
use tokio::sync::{broadcast, oneshot};
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
    let ops_config = DbOpsConfig::new(config.client.db_retry_count);

    // initialize core databases
    let database = init_core_dbs(rbdb.clone(), ops_config);

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
        ops_config,
    ));
    let bridge_msg_ctx = express_storage::ops::bridge_relay::Context::new(bridge_msg_db);
    let bridge_msg_ops = Arc::new(bridge_msg_ctx.into_ops(pool.clone()));

    let checkpoint_manager: Arc<_> =
        CheckpointDbManager::new(pool.clone(), database.clone()).into();
    let checkpoint_handle: Arc<_> = CheckpointHandle::new(checkpoint_manager.clone()).into();
    let bitcoin_client = create_bitcoin_rpc_client(&config)?;

    let l2_block_manager = Arc::new(L2BlockManager::new(pool.clone(), database.clone()));

    // Check if we have to do genesis.
    if genesis::check_needs_client_init(database.as_ref())? {
        info!("need to init client state!");
        genesis::init_client_state(&params, database.as_ref())?;
    }

    // init status tasks
    let (status_tx, status_rx) = init_status_channel(database.as_ref())?;

    let engine_ctl = init_engine_controller(
        &config,
        database.clone(),
        params.as_ref(),
        l2_block_manager.clone(),
        &rt,
    )?;

    // Start the sync manager.
    let sync_manager: Arc<_> = sync_manager::start_sync_tasks(
        &executor,
        database.clone(),
        l2_block_manager.clone(),
        engine_ctl.clone(),
        pool.clone(),
        params.clone(),
        (status_tx.clone(), status_rx.clone()),
        checkpoint_manager,
    )?
    .into();

    // Start the L1 tasks to get that going.
    l1_reader::start_reader_tasks(
        &executor,
        sync_manager.get_params(),
        &config,
        bitcoin_client.clone(),
        database.clone(),
        sync_manager.get_csm_ctl(),
        status_tx.clone(),
    )?;

    // Start relayer task.
    let relayer_handle = express_bridge_relay::relayer::start_bridge_relayer_task(
        bridge_msg_ops,
        status_rx.clone(),
        config.relayer,
        &executor,
    );

    // If we're a sequencer, start the sequencer db and duties task.
    if let ClientMode::Sequencer(sequencer_config) = &config.client.client_mode {
        let broadcast_database = init_broadcaster_database(rbdb.clone(), ops_config);
        let broadcast_handle = start_broadcaster_tasks(
            broadcast_database,
            pool.clone(),
            &executor,
            bitcoin_client.clone(),
        );
        let seq_db = init_sequencer_database(rbdb.clone(), ops_config);

        start_sequencer_tasks(
            sequencer_config,
            &config,
            params.clone(),
            bitcoin_client,
            &executor,
            seq_db,
            sync_manager.clone(),
            database.clone(),
            engine_ctl.clone(),
            pool.clone(),
            l2_block_manager.clone(),
            status_tx,
            checkpoint_handle.clone(),
            broadcast_handle,
            &mut methods,
        )?;
    };

    if let ClientMode::FullNode(fullnode_config) = &config.client.client_mode {
        let sequencer_rpc = &fullnode_config.sequencer_rpc;
        info!(?sequencer_rpc, "initing fullnode task");

        let rpc_client = rt.block_on(sync_client(sequencer_rpc));
        let sync_peer = RpcSyncPeer::new(rpc_client, 10);
        let l2_sync_context =
            L2SyncContext::new(sync_peer, l2_block_manager.clone(), sync_manager.clone());
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

    executor.spawn_critical_async(
        "main-rpc",
        start_rpc(
            task_manager.shutdown_signal(),
            config,
            sync_manager,
            database,
            status_rx,
            l2_block_manager,
            checkpoint_handle,
            relayer_handle,
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
    sync_manager: Arc<SyncManager>,
    database: Arc<D>,
    status_rx: Arc<StatusRx>,
    l2_block_manager: Arc<L2BlockManager>,
    checkpoint_handle: Arc<CheckpointHandle>,
    relayer_handle: Arc<RelayerHandle>,
    mut methods: Methods,
) -> anyhow::Result<()>
where
    D: Database + Send + Sync + 'static,
{
    let (stop_tx, stop_rx) = oneshot::channel();

    // Init RPC impls.
    let alp_rpc = rpc_server::AlpenRpcImpl::new(
        status_rx,
        database,
        sync_manager,
        l2_block_manager,
        checkpoint_handle,
        relayer_handle,
    );
    methods.merge(alp_rpc.into_rpc())?;

    let admin_rpc = rpc_server::AdminServerImpl::new(stop_tx);
    methods.merge(admin_rpc.into_rpc())?;

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

pub struct CoreContext {
    pub db: Arc<CommonDb>,
    pub pool: threadpool::ThreadPool,
    pub params: Arc<Params>,
    pub sync_manager: Arc<SyncManager>,
    pub l2block_manager: Arc<L2BlockManager>,
    pub status_tx: Arc<StatusTx>,
    pub status_rx: Arc<StatusRx>,
    pub engine_ctl: Arc<RpcExecEngineCtl<EngineRpcClient>>,
}

#[allow(clippy::too_many_arguments)]
fn start_sequencer_tasks<E: ExecEngineCtl + Send + Sync + 'static>(
    seq_config: &SequencerConfig,
    config: &Config,
    params: Arc<Params>,
    bitcoin_client: Arc<BitcoinClient>,
    executor: &TaskExecutor,
    seq_db: Arc<SequencerDB<RBSeqBlobDb>>,
    sync_manager: Arc<SyncManager>,
    database: Arc<CommonDb>,
    engine_ctl: Arc<E>,
    pool: threadpool::ThreadPool,
    l2_block_manager: Arc<L2BlockManager>,
    status_tx: Arc<StatusTx>,
    checkpoint_handle: Arc<CheckpointHandle>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    methods: &mut Methods,
) -> anyhow::Result<()> {
    info!(seqkey_path = ?seq_config.sequencer_key, "initing sequencer duties task");
    let idata = load_seqkey(&seq_config.sequencer_key)?;

    // Set up channel and clone some things.
    let (duties_tx, duties_rx) = broadcast::channel::<DutyBatch>(8);

    // Spawn up writer
    let writer_config = WriterConfig::new(
        seq_config.sequencer_bitcoin_address.clone(),
        config.bitcoind_rpc.network,
        params.rollup().rollup_name.clone(),
    )?;

    // Start inscription tasks
    let inscription_handle = start_inscription_task(
        executor,
        bitcoin_client,
        writer_config,
        seq_db,
        status_tx.clone(),
        pool.clone(),
        broadcast_handle.clone(),
    )?;

    let admin_rpc = rpc_server::SequencerServerImpl::new(
        inscription_handle.clone(),
        broadcast_handle,
        params.clone(),
        checkpoint_handle.clone(),
    );
    methods.merge(admin_rpc.into_rpc())?;

    // Spawn duty tasks.
    let cupdate_rx = sync_manager.create_cstate_subscription();
    let t_l2_block_manager = l2_block_manager.clone();
    let t_params = params.clone();
    let t_database = database.clone();
    executor.spawn_critical("duty_worker::duty_tracker_task", move |shutdown| {
        duty_worker::duty_tracker_task(
            shutdown,
            cupdate_rx,
            duties_tx,
            idata.ident,
            t_database,
            t_l2_block_manager,
            t_params,
        )
        .map_err(Into::into)
    });

    let d_executor = executor.clone();
    executor.spawn_critical("duty_worker::duty_dispatch_task", move |shutdown| {
        duty_worker::duty_dispatch_task(
            shutdown,
            d_executor,
            duties_rx,
            idata.key,
            sync_manager,
            database,
            engine_ctl,
            inscription_handle,
            pool,
            params,
            checkpoint_handle,
        )
    });

    Ok(())
}

fn start_broadcaster_tasks(
    broadcast_database: Arc<BroadcastDatabase>,
    pool: threadpool::ThreadPool,
    executor: &TaskExecutor,
    bitcoin_client: Arc<BitcoinClient>,
) -> Arc<L1BroadcastHandle> {
    // Set up L1 broadcaster.
    let broadcast_ctx = express_storage::ops::l1tx_broadcast::Context::new(broadcast_database);
    let broadcast_ops = Arc::new(broadcast_ctx.into_ops(pool));
    // start broadcast task
    let broadcast_handle = spawn_broadcaster_task(executor, bitcoin_client.clone(), broadcast_ops);
    Arc::new(broadcast_handle)
}
