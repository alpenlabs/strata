#![allow(dead_code)] // TODO: remove this once `Args.network` is used
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use alpen_express_btcio::{
    broadcaster::{spawn_broadcaster_task, L1BroadcastHandle},
    rpc::BitcoinClient,
    writer::{config::WriterConfig, start_inscription_task, InscriptionHandle},
};
use alpen_express_common::logging;
use alpen_express_consensus_logic::{
    checkpoint::CheckpointHandle,
    duty::{
        types::{DutyBatch, Identity, IdentityData, IdentityKey},
        worker::{self as duty_worker},
    },
    genesis, state_tracker, sync_manager,
    sync_manager::SyncManager,
};
use alpen_express_db::traits::Database;
use alpen_express_evmexec::{fork_choice_state_initial, EngineRpcClient};
use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, RollupParams, SyncParams},
    vk::RollupVerifyingKey,
};
use alpen_express_rocksdb::{
    broadcaster::db::BroadcastDatabase, sequencer::db::SequencerDB, DbOpsConfig, SeqDb,
};
use alpen_express_rpc_api::{AlpenAdminApiServer, AlpenApiServer};
use alpen_express_rpc_types::L1Status;
use alpen_express_state::csm_status::CsmStatus;
use alpen_express_status::{create_status_channel, StatusRx, StatusTx};
use anyhow::Context;
use bitcoin::Network;
use config::{ClientMode, Config};
use express_bridge_relay::relayer::RelayerHandle;
use express_storage::{managers::checkpoint::CheckpointDbManager, L2BlockManager};
use express_sync::{self, L2SyncContext, RpcSyncPeer};
use express_tasks::{ShutdownSignal, TaskManager};
use format_serde_error::SerdeError;
use reth_rpc_types::engine::{JwtError, JwtSecret};
use rockbound::rocksdb;
use rpc_client::sync_client;
use thiserror::Error;
use tokio::sync::{broadcast, oneshot};
use tracing::*;

use crate::args::Args;

mod args;
mod config;
mod l1_reader;
mod rpc_client;
mod rpc_server;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("config: {0}")]
    MalformedConfig(#[from] SerdeError),

    #[error("jwt: {0}")]
    MalformedSecret(#[from] JwtError),

    #[error("{0}")]
    Other(String),
}

fn load_configuration(path: &Path) -> Result<Config, InitError> {
    let config_str = fs::read_to_string(path)?;
    let conf = toml::from_str::<Config>(&config_str)
        .map_err(|err| SerdeError::new(config_str.to_string(), err))?;
    Ok(conf)
}

fn load_jwtsecret(path: &Path) -> Result<JwtSecret, InitError> {
    let secret = fs::read_to_string(path)?;
    let jwt_secret = JwtSecret::from_hex(secret)?;

    Ok(jwt_secret)
}

fn load_rollup_params_or_default(path: &Option<PathBuf>) -> Result<RollupParams, InitError> {
    match path {
        Some(path) => {
            let json = fs::read_to_string(path)?;
            let rollup_params = serde_json::from_str::<RollupParams>(&json)
                .map_err(|err| SerdeError::new(json.to_string(), err))?;

            Ok(rollup_params)
        }
        None => Ok(default_rollup_params()),
    }
}

fn default_rollup_params() -> RollupParams {
    // FIXME this is broken, where are the keys?
    let opkeys = OperatorPubkeys::new(Buf32::zero(), Buf32::zero());

    // TODO: load default params from a json during compile time
    RollupParams {
        rollup_name: "express".to_string(),
        block_time: 1000,
        cred_rule: block_credential::CredRule::Unchecked,
        horizon_l1_height: 3,
        genesis_l1_height: 5,
        operator_config: OperatorConfig::Static(vec![opkeys]),
        evm_genesis_block_hash: Buf32(
            "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                .parse()
                .unwrap(),
        ),
        evm_genesis_block_state_root: Buf32(
            "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                .parse()
                .unwrap(),
        ),
        l1_reorg_safe_depth: 4,
        target_l2_batch_size: 64,
        address_length: 20,
        deposit_amount: 1_000_000_000,
        rollup_vk: RollupVerifyingKey::SP1VerifyingKey(Buf32(
            "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
                .parse()
                .unwrap(),
        )), // TODO: update this with vk for checkpoint proof
        verify_proofs: true,
        dispatch_assignment_dur: 64,
    }
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("FATAL ERROR: {e}");
        eprintln!("trace:\n{e:?}");
        // TODO: error code ?

        return Err(e);
    }

    Ok(())
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    logging::init();

    // initialize the full configuration
    let config = match args.config.as_ref() {
        Some(config_path) => {
            // Values passed over arguments get the precedence over the configuration files
            let mut config = load_configuration(config_path)?;
            config.update_from_args(&args);
            config
        }
        None => match Config::from_args(&args) {
            Err(msg) => {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }
            Ok(cfg) => cfg,
        },
    };

    // Open the database.
    let rbdb = open_rocksdb_database(&config)?;
    // init a database configuration
    let db_ops = DbOpsConfig {
        retry_count: config.client.db_retry_count,
    };

    // Set up block params.
    let params = Params {
        rollup: load_rollup_params_or_default(&args.rollup_params).expect("rollup params"),
        run: SyncParams {
            l1_follow_distance: config.sync.l1_follow_distance,
            client_checkpoint_interval: config.sync.client_checkpoint_interval,
            l2_blocks_fetch_limit: config.client.l2_blocks_fetch_limit,
        },
    };

    let params = Arc::new(params);

    // Start runtime for async IO tasks.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("express-rt")
        .build()
        .expect("init: build rt");

    // Init thread pool for batch jobs.
    // TODO switch to num_cpus maybe?  we don't want to compete with tokio though
    let pool = threadpool::ThreadPool::with_name("express-pool".to_owned(), 8);

    let task_manager = TaskManager::new(rt.handle().clone());
    let task_executor = task_manager.executor();

    // Initialize databases.
    let l1_db = Arc::new(alpen_express_rocksdb::L1Db::new(rbdb.clone(), db_ops));
    let l2_db = Arc::new(alpen_express_rocksdb::l2::db::L2Db::new(
        rbdb.clone(),
        db_ops,
    ));
    let sync_ev_db = Arc::new(alpen_express_rocksdb::SyncEventDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let cs_db = Arc::new(alpen_express_rocksdb::ClientStateDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let chst_db = Arc::new(alpen_express_rocksdb::ChainStateDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let bcast_db = Arc::new(alpen_express_rocksdb::BroadcastDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let checkpt_db = Arc::new(alpen_express_rocksdb::RBCheckpointDB::new(
        rbdb.clone(),
        db_ops,
    ));
    let database = Arc::new(alpen_express_db::database::CommonDatabase::new(
        l1_db, l2_db, sync_ev_db, cs_db, chst_db, checkpt_db,
    ));

    // Set up bridge messaging stuff.
    // TODO move all of this into relayer task init
    let bridge_msg_db = Arc::new(alpen_express_rocksdb::BridgeMsgDb::new(
        rbdb.clone(),
        db_ops,
    ));
    let bridge_msg_ctx = express_storage::ops::bridge_relay::Context::new(bridge_msg_db);
    let bridge_msg_ops = Arc::new(bridge_msg_ctx.into_ops(pool.clone()));

    // Set up database managers.
    let l2_block_manager = Arc::new(L2BlockManager::new(pool.clone(), database.clone()));
    let checkpoint_manager = Arc::new(CheckpointDbManager::new(pool.clone(), database.clone()));
    let checkpoint_handle = Arc::new(CheckpointHandle::new(checkpoint_manager.clone()));

    // Set up Bitcoin client RPC.
    let bitcoind_url = format!("http://{}", config.bitcoind_rpc.rpc_url);
    let btc_rpc = BitcoinClient::new(
        bitcoind_url,
        config.bitcoind_rpc.rpc_user.clone(),
        config.bitcoind_rpc.rpc_password.clone(),
    )
    .map_err(anyhow::Error::from)?;
    let btc_rpc = Arc::new(btc_rpc);

    // TODO remove this
    if config.bitcoind_rpc.network != Network::Regtest {
        warn!("network not set to regtest, ignoring");
    }

    // Init engine controller.
    let reth_jwtsecret = load_jwtsecret(&config.exec.reth.secret)?;
    let client = EngineRpcClient::from_url_secret(
        &format!("http://{}", &config.exec.reth.rpc_url),
        reth_jwtsecret,
    );

    let initial_fcs = fork_choice_state_initial(database.clone(), params.rollup())?;
    let eng_ctl = alpen_express_evmexec::engine::RpcExecEngineCtl::new(
        client,
        initial_fcs,
        rt.handle().clone(),
        l2_block_manager.clone(),
    );
    let eng_ctl = Arc::new(eng_ctl);

    // Set up L1 broadcaster.
    let bcastdb = Arc::new(BroadcastDatabase::new(bcast_db));
    let bcast_ctx = express_storage::ops::l1tx_broadcast::Context::new(bcastdb.clone());
    let bcast_ops = Arc::new(bcast_ctx.into_ops(pool.clone()));
    //status bundles
    let (status_tx, status_rx) = start_status(database.clone(), params.clone())?;

    // Start the sync manager.
    let sync_man = sync_manager::start_sync_tasks(
        &task_executor,
        database.clone(),
        l2_block_manager.clone(),
        eng_ctl.clone(),
        pool.clone(),
        params.clone(),
        (status_tx.clone(), status_rx.clone()),
        checkpoint_manager.clone(),
    )?;
    let sync_man = Arc::new(sync_man);
    let mut inscription_handler = None;

    // Start broadcast task.
    let bcast_handle = spawn_broadcaster_task(&task_executor, btc_rpc.clone(), bcast_ops);
    let bcast_handle = Arc::new(bcast_handle);
    let (status_tx, status_rx) = (sync_man.status_tx(), sync_man.status_rx());

    // Start relayer task.
    // TODO cleanup, this is ugly
    let start_relayer_fut = express_bridge_relay::relayer::start_bridge_relayer_task(
        bridge_msg_ops,
        status_rx.clone(),
        config.relayer,
        &task_executor,
    );

    // FIXME this init is screwed up because of the order we start things
    let relayer_handle = rt.block_on(start_relayer_fut)?;

    // If the sequencer key is set, start the sequencer duties task.
    if let ClientMode::Sequencer(sequencer_config) = &config.client.client_mode {
        let seqkey_path = &sequencer_config.sequencer_key;
        info!(?seqkey_path, "initing sequencer duties task");
        let idata = load_seqkey(seqkey_path)?;
        let executor = task_manager.executor();

        // Set up channel and clone some things.
        let sm = sync_man.clone();
        let cu_rx = sync_man.create_cstate_subscription();
        let (duties_tx, duties_rx) = broadcast::channel::<DutyBatch>(8);
        let db = database.clone();
        let db2 = database.clone();
        let eng_ctl_de = eng_ctl.clone();
        let pool = pool.clone();

        // Spawn up writer
        let writer_config = WriterConfig::new(
            sequencer_config.sequencer_bitcoin_address.clone(),
            config.bitcoind_rpc.network,
            params.rollup().rollup_name.clone(),
        )?;

        // Initialize SequencerDatabase
        let seqdb = Arc::new(SeqDb::new(rbdb, db_ops));
        let dbseq = Arc::new(SequencerDB::new(seqdb));
        let rpc = btc_rpc.clone();

        // Start inscription tasks
        let insc_hndlr = Arc::new(start_inscription_task(
            &task_executor,
            rpc,
            writer_config,
            dbseq.clone(),
            status_tx.clone(),
            pool.clone(),
            bcast_handle.clone(),
        )?);

        inscription_handler = Some(insc_hndlr.clone());

        // Spawn duty tasks.
        let t_l2blkman = l2_block_manager.clone();
        let t_params = params.clone();
        executor.spawn_critical("duty_worker::duty_tracker_task", move |shutdown| {
            duty_worker::duty_tracker_task(
                shutdown,
                cu_rx,
                duties_tx,
                idata.ident,
                db,
                t_l2blkman,
                t_params,
            )
            .unwrap();
        });

        let d_params = params.clone();
        let d_executor = task_manager.executor();
        let checkpt_h = checkpoint_handle.clone();
        executor.spawn_critical("duty_worker::duty_dispatch_task", move |shutdown| {
            duty_worker::duty_dispatch_task(
                shutdown, d_executor, duties_rx, idata.key, sm, db2, eng_ctl_de, insc_hndlr, pool,
                d_params, checkpt_h,
            )
        });
    }

    // Start the L1 tasks to get that going.
    let csm_ctl = sync_man.get_csm_ctl();
    l1_reader::start_reader_tasks(
        &task_executor,
        sync_man.get_params(),
        &config,
        btc_rpc.clone(),
        database.clone(),
        csm_ctl,
        status_tx.clone(),
    )?;

    if let ClientMode::FullNode(fullnode_config) = &config.client.client_mode {
        let sequencer_rpc = &fullnode_config.sequencer_rpc;
        info!(?sequencer_rpc, "initing fullnode task");

        let rpc_client = rt.block_on(sync_client(sequencer_rpc));
        let sync_peer = RpcSyncPeer::new(rpc_client, 10);
        let l2_sync_context =
            L2SyncContext::new(sync_peer, l2_block_manager.clone(), sync_man.clone());
        // NOTE: this might block for some time during first run with empty db until genesis block
        // is generated
        let mut l2_sync_state =
            express_sync::block_until_csm_ready_and_init_sync_state(&l2_sync_context)?;

        task_executor.spawn_critical_async("l2-sync-manager", async move {
            express_sync::sync_worker(&mut l2_sync_state, &l2_sync_context)
                .await
                .unwrap();
        });
    }

    let shutdown_signal = task_manager.shutdown_signal();
    let db_cloned = database.clone();

    let rpc_params = params.clone();
    task_executor.spawn_critical_async("main-rpc", async {
        start_rpc(
            shutdown_signal,
            config,
            sync_man,
            db_cloned,
            status_rx,
            inscription_handler,
            bcast_handle,
            l2_block_manager,
            checkpoint_handle,
            relayer_handle,
            rpc_params,
        )
        .await
        .unwrap()
    });

    task_manager.start_signal_listeners();
    if let Err(err) = task_manager.monitor(Some(Duration::from_secs(5))) {
        // we exited because of a panic
        return Err(anyhow::Error::from(err));
    }

    info!("exiting");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_rpc<D: Database + Send + Sync + 'static>(
    shutdown_signal: ShutdownSignal,
    config: Config,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    status_rx: Arc<StatusRx>,
    inscription_handler: Option<Arc<InscriptionHandle>>,
    bcast_handle: Arc<L1BroadcastHandle>,
    l2_block_manager: Arc<L2BlockManager>,
    checkpt_handle: Arc<CheckpointHandle>,
    relayer_handle: Arc<RelayerHandle>,
    params: Arc<Params>,
) -> anyhow::Result<()> {
    let (stop_tx, stop_rx) = oneshot::channel();

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

    let admin_rpc = rpc_server::AdminServerImpl::new(
        inscription_handler,
        bcast_handle,
        stop_tx,
        params,
        checkpt_handle.clone(),
    );

    // Construct the full methods table.
    let mut methods = alp_rpc.into_rpc();
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

// initializes the status bundle that we can pass around cheaply for as name suggests status/metrics
// FIXME this is just supposed to handle the status trackers, why are we doing database init here?
fn start_status<D: Database + Send + Sync + 'static>(
    database: Arc<D>,
    params: Arc<Params>,
) -> anyhow::Result<(Arc<StatusTx>, Arc<StatusRx>)>
where
    <D as Database>::CsProv: Send + Sync + 'static,
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
