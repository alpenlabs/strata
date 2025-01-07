use std::{str::FromStr, sync::Arc, time::Duration};

use bitcoin::{hashes::Hash, Address, BlockHash};
use jsonrpsee::Methods;
use rpc_client::sync_client;
use strata_bridge_relay::relayer::RelayerHandle;
use strata_btcio::{
    broadcaster::{spawn_broadcaster_task, L1BroadcastHandle},
    rpc::{traits::Reader, BitcoinClient},
    writer::{config::WriterConfig, start_inscription_task},
};
use strata_common::logging;
use strata_config::{ClientMode, Config, SequencerConfig};
use strata_consensus_logic::{
    checkpoint::CheckpointHandle,
    duty::{types::DutyBatch, worker as duty_worker},
    genesis,
    sync_manager::{self, SyncManager},
};
use strata_db::{
    traits::{BroadcastDatabase, ChainstateDatabase, Database},
    DbError,
};
use strata_eectl::engine::ExecEngineCtl;
use strata_evmexec::{engine::RpcExecEngineCtl, EngineRpcClient};
use strata_primitives::params::Params;
use strata_rocksdb::{
    broadcaster::db::BroadcastDb, init_broadcaster_database, init_core_dbs,
    init_sequencer_database, open_rocksdb_database, sequencer::db::SequencerDB, CommonDb,
    DbOpsConfig, RBSeqBlobDb, ROCKSDB_NAME,
};
use strata_rpc_api::{
    StrataAdminApiServer, StrataApiServer, StrataDebugApiServer, StrataSequencerApiServer,
};
use strata_status::StatusChannel;
use strata_storage::{
    create_node_storage, ops::bridge_relay::BridgeMsgOps, L2BlockManager, NodeStorage,
};
use strata_sync::{self, L2SyncContext, RpcSyncPeer};
use strata_tasks::{ShutdownSignal, TaskExecutor, TaskManager};
use tokio::{
    runtime::Handle,
    sync::{broadcast, oneshot},
};
use tracing::*;

use crate::{args::Args, helpers::*};

mod args;
mod errors;
mod extractor;
mod helpers;
mod l1_reader;
mod network;
mod rpc_client;
mod rpc_server;

// TODO: this might need to come from config.
const BITCOIN_POLL_INTERVAL: u64 = 200; // millis
const SEQ_ADDR_GENERATION_TIMEOUT: u64 = 10; // seconds

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
    // Load and validate configuration and params
    let config = get_config(args.clone())?;
    // Set up block params.
    let params = resolve_and_validate_params(args.rollup_params.as_deref(), &config)
        .map_err(anyhow::Error::from)?;

    // Init the task manager and logging before we do anything else.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("strata-rt")
        .build()
        .expect("init: build rt");
    let task_manager = TaskManager::new(runtime.handle().clone());
    let executor = task_manager.executor();

    init_logging(executor.handle());

    // Init thread pool for batch jobs.
    // TODO switch to num_cpus
    let pool = threadpool::ThreadPool::with_name("strata-pool".to_owned(), 8);

    // Open and initialize rocksdb.
    let rbdb = open_rocksdb_database(&config.client.datadir, ROCKSDB_NAME)?;
    let ops_config = DbOpsConfig::new(config.client.db_retry_count);

    // Initialize core databases
    let database = init_core_dbs(rbdb.clone(), ops_config);
    let manager = create_node_storage(database.clone(), pool.clone());

    // Set up bridge messaging stuff.
    // TODO move all of this into relayer task init
    let bridge_msg_db = Arc::new(strata_rocksdb::BridgeMsgDb::new(rbdb.clone(), ops_config));
    let bridge_msg_ctx = strata_storage::ops::bridge_relay::Context::new(bridge_msg_db);
    let bridge_msg_ops = Arc::new(bridge_msg_ctx.into_ops(pool.clone()));

    let checkpoint_handle: Arc<_> = CheckpointHandle::new(manager.checkpoint().clone()).into();
    let bitcoin_client = create_bitcoin_rpc_client(&config)?;

    // Check if we have to do genesis.
    if genesis::check_needs_client_init(database.as_ref())? {
        info!("need to init client state!");
        genesis::init_client_state(&params, database.as_ref())?;
    }

    info!("init finished, starting main tasks");

    let ctx = start_core_tasks(
        &executor,
        pool,
        &config,
        params.clone(),
        database,
        &manager,
        bridge_msg_ops,
        bitcoin_client,
    )?;

    let mut methods = jsonrpsee::Methods::new();

    match &config.client.client_mode {
        // If we're a sequencer, start the sequencer db and duties task.
        ClientMode::Sequencer(sequencer_config) => {
            let broadcast_database = init_broadcaster_database(rbdb.clone(), ops_config);
            let broadcast_handle = start_broadcaster_tasks(
                broadcast_database,
                ctx.pool.clone(),
                &executor,
                ctx.bitcoin_client.clone(),
                params.clone(),
            );
            let seq_db = init_sequencer_database(rbdb.clone(), ops_config);

            start_sequencer_tasks(
                ctx.clone(),
                &config,
                sequencer_config,
                &executor,
                seq_db,
                checkpoint_handle.clone(),
                broadcast_handle,
                &mut methods,
            )?;
        }
        ClientMode::FullNode(fullnode_config) => {
            let sequencer_rpc = &fullnode_config.sequencer_rpc;
            info!(?sequencer_rpc, "initing fullnode task");

            let rpc_client = runtime.block_on(sync_client(sequencer_rpc));
            let sync_peer = RpcSyncPeer::new(rpc_client, 10);
            let l2_sync_context = L2SyncContext::new(
                sync_peer,
                ctx.l2_block_manager.clone(),
                ctx.sync_manager.clone(),
            );
            // NOTE: this might block for some time during first run with empty db until genesis
            // block is generated
            let mut l2_sync_state =
                strata_sync::block_until_csm_ready_and_init_sync_state(&l2_sync_context)?;

            executor.spawn_critical_async("l2-sync-manager", async move {
                strata_sync::sync_worker(&mut l2_sync_state, &l2_sync_context)
                    .await
                    .map_err(Into::into)
            });
        }
    }

    executor.spawn_critical_async(
        "main-rpc",
        start_rpc(
            ctx,
            task_manager.shutdown_signal(),
            config,
            checkpoint_handle,
            methods,
        ),
    );

    task_manager.start_signal_listeners();
    task_manager.monitor(Some(Duration::from_secs(5)))?;

    info!("exiting");
    Ok(())
}

/// Sets up the logging system given a handle to a runtime context to possibly
/// start the OTLP output on.
fn init_logging(rt: &Handle) {
    let mut lconfig = logging::LoggerConfig::with_base_name("strata-client");

    // Set the OpenTelemetry URL if set.
    let otlp_url = logging::get_otlp_url_from_env();
    if let Some(url) = &otlp_url {
        lconfig.set_otlp_url(url.clone());
    }

    {
        // Need to set the runtime context because of nonsense.
        let _g = rt.enter();
        logging::init(lconfig);
    }

    // Have to log this after we start the logging formally.
    if let Some(url) = &otlp_url {
        info!(%url, "using OpenTelemetry tracing output");
    }
}

#[derive(Clone)]
pub struct CoreContext {
    pub database: Arc<CommonDb>,
    pub pool: threadpool::ThreadPool,
    pub params: Arc<Params>,
    pub sync_manager: Arc<SyncManager>,
    pub l2_block_manager: Arc<L2BlockManager>,
    pub status_channel: StatusChannel,
    pub engine: Arc<RpcExecEngineCtl<EngineRpcClient>>,
    pub relayer_handle: Arc<RelayerHandle>,
    pub bitcoin_client: Arc<BitcoinClient>,
}

fn do_startup_checks(
    database: &impl Database,
    engine: &impl ExecEngineCtl,
    bitcoin_client: &impl Reader,
    handle: &Handle,
) -> anyhow::Result<()> {
    let chain_state_db = database.chain_state_db();
    let last_state_idx = match chain_state_db.get_last_state_idx() {
        Ok(idx) => idx,
        Err(DbError::NotBootstrapped) => {
            // genesis is not done
            info!("startup: awaiting genesis");
            return Ok(());
        }
        err => err?,
    };
    let Some(last_chain_state) = chain_state_db.get_toplevel_state(last_state_idx)? else {
        anyhow::bail!(format!("Missing chain state idx: {}", last_state_idx));
    };

    // Check that we can connect to bitcoin client and block we believe to be matured in L1 is
    // actually present
    let safe_l1blockid = last_chain_state.l1_view().safe_block().blkid();
    let block_hash = BlockHash::from_slice(safe_l1blockid.as_ref())?;

    match handle.block_on(bitcoin_client.get_block(&block_hash)) {
        Ok(_block) => {
            info!("startup: last matured block: {}", block_hash);
        }
        Err(client_error) if client_error.is_block_not_found() => {
            anyhow::bail!("Missing expected block: {}", block_hash);
        }
        Err(client_error) => {
            anyhow::bail!("could not connect to bitcoin, err = {}", client_error);
        }
    }

    // Check that tip L2 block exists (and engine can be connected to)
    let chain_tip = last_chain_state.chain_tip_blockid();
    match engine.check_block_exists(chain_tip) {
        Ok(true) => {
            info!("startup: last l2 block is synced")
        }
        Ok(false) => {
            // Current chain tip tip block is not known by the EL.
            // TODO: Try to sync EL using existing block payloads from DB.
            anyhow::bail!("missing expected evm block, block_id = {}", chain_tip);
        }
        Err(error) => {
            // Likely network issue
            anyhow::bail!("could not connect to exec engine, err = {}", error);
        }
    }

    // everything looks ok
    info!("Startup checks passed");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn start_core_tasks(
    executor: &TaskExecutor,
    pool: threadpool::ThreadPool,
    config: &Config,
    params: Arc<Params>,
    database: Arc<CommonDb>,
    storage: &NodeStorage,
    bridge_msg_ops: Arc<BridgeMsgOps>,
    bitcoin_client: Arc<BitcoinClient>,
) -> anyhow::Result<CoreContext> {
    // init status tasks
    let status_channel = init_status_channel(database.as_ref())?;

    let engine = init_engine_controller(
        config,
        database.clone(),
        params.as_ref(),
        storage.l2().clone(),
        executor.handle(),
    )?;

    // do startup checks
    do_startup_checks(
        database.as_ref(),
        engine.as_ref(),
        bitcoin_client.as_ref(),
        executor.handle(),
    )?;

    // Start the sync manager.
    let sync_manager: Arc<_> = sync_manager::start_sync_tasks(
        executor,
        database.clone(),
        storage,
        engine.clone(),
        pool.clone(),
        params.clone(),
        status_channel.clone(),
    )?
    .into();

    // Start the L1 tasks to get that going.
    l1_reader::start_reader_tasks(
        executor,
        sync_manager.get_params(),
        config,
        bitcoin_client.clone(),
        database.clone(),
        sync_manager.get_csm_ctl(),
        status_channel.clone(),
    )?;

    // Start relayer task.
    let relayer_handle = strata_bridge_relay::relayer::start_bridge_relayer_task(
        bridge_msg_ops,
        status_channel.clone(),
        config.relayer,
        executor,
    );

    Ok(CoreContext {
        database,
        pool,
        params,
        sync_manager,
        l2_block_manager: storage.l2().clone(),
        status_channel,
        engine,
        relayer_handle,
        bitcoin_client,
    })
}

#[allow(clippy::too_many_arguments)]
fn start_sequencer_tasks(
    ctx: CoreContext,
    config: &Config,
    sequencer_config: &SequencerConfig,
    executor: &TaskExecutor,
    seq_db: Arc<SequencerDB<RBSeqBlobDb>>,
    checkpoint_handle: Arc<CheckpointHandle>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    methods: &mut Methods,
) -> anyhow::Result<()> {
    let CoreContext {
        database,
        pool,
        params,
        sync_manager,
        l2_block_manager,
        status_channel,
        engine,
        bitcoin_client,
        ..
    } = ctx;

    info!(seqkey_path = ?sequencer_config.sequencer_key, "initing sequencer duties task");
    let idata = load_seqkey(&sequencer_config.sequencer_key)?;

    // Set up channel and clone some things.
    let (duties_tx, duties_rx) = broadcast::channel::<DutyBatch>(8);

    // Use provided address or generate an address owned by the sequencer's bitcoin wallet
    let sequencer_bitcoin_address = match sequencer_config.sequencer_bitcoin_address.as_ref() {
        Some(address) => {
            Address::from_str(address)?.require_network(config.bitcoind_rpc.network)?
        }
        None => executor.handle().block_on(generate_sequencer_address(
            &bitcoin_client,
            SEQ_ADDR_GENERATION_TIMEOUT,
            BITCOIN_POLL_INTERVAL,
        ))?,
    };

    // Spawn up writer
    let writer_config = WriterConfig::new(
        sequencer_bitcoin_address,
        params.rollup().rollup_name.clone(),
    )?;

    // Start inscription tasks
    let inscription_handle = start_inscription_task(
        executor,
        bitcoin_client,
        writer_config,
        seq_db,
        status_channel.clone(),
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
            engine,
            inscription_handle,
            pool,
            params,
            checkpoint_handle,
        )
    });

    Ok(())
}

fn start_broadcaster_tasks(
    broadcast_database: Arc<BroadcastDb>,
    pool: threadpool::ThreadPool,
    executor: &TaskExecutor,
    bitcoin_client: Arc<BitcoinClient>,
    params: Arc<Params>,
) -> Arc<L1BroadcastHandle> {
    // Set up L1 broadcaster.
    let broadcast_ctx = strata_storage::ops::l1tx_broadcast::Context::new(
        broadcast_database.l1_broadcast_db().clone(),
    );
    let broadcast_ops = Arc::new(broadcast_ctx.into_ops(pool));
    // start broadcast task
    let broadcast_handle =
        spawn_broadcaster_task(executor, bitcoin_client.clone(), broadcast_ops, params);
    Arc::new(broadcast_handle)
}

async fn start_rpc(
    ctx: CoreContext,
    shutdown_signal: ShutdownSignal,
    config: Config,
    checkpoint_handle: Arc<CheckpointHandle>,
    mut methods: Methods,
) -> anyhow::Result<()> {
    let CoreContext {
        database,
        sync_manager,
        l2_block_manager,
        status_channel,
        relayer_handle,
        ..
    } = ctx;

    let (stop_tx, stop_rx) = oneshot::channel();

    // Init RPC impls.
    let strata_rpc = rpc_server::StrataRpcImpl::new(
        status_channel.clone(),
        database.clone(),
        sync_manager,
        l2_block_manager.clone(),
        checkpoint_handle,
        relayer_handle,
    );
    methods.merge(strata_rpc.into_rpc())?;

    let admin_rpc = rpc_server::AdminServerImpl::new(stop_tx);
    methods.merge(admin_rpc.into_rpc())?;

    let debug_rpc = rpc_server::StrataDebugRpcImpl::new(l2_block_manager, database);
    methods.merge(debug_rpc.into_rpc())?;

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
