use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use alpen_express_btcio::{
    broadcaster::{spawn_broadcaster_task, L1BroadcastHandle},
    rpc::BitcoinClient,
    writer::{config::WriterConfig, start_inscription_task, InscriptionHandle},
};
use alpen_express_consensus_logic::{
    checkpoint::CheckpointHandle,
    duty::{
        types::{DutyBatch, Identity, IdentityData, IdentityKey},
        worker as duty_worker,
    },
    sync_manager::{self, SyncManager},
};
use alpen_express_db::database::CommonDatabase;
use alpen_express_evmexec::{engine::RpcExecEngineCtl, fork_choice_state_initial, EngineRpcClient};
use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, ProofPublishMode, RollupParams},
    vk::RollupVerifyingKey,
};
use alpen_express_rocksdb::{
    broadcaster::db::BroadcastDatabase, l2::db::L2Db, sequencer::db::SequencerDB, BroadcastDb,
    ChainStateDb, ClientStateDb, DbOpsConfig, L1Db, RBCheckpointDB, SeqDb, SyncEventDb,
};
use alpen_express_status::{StatusRx, StatusTx};
use bitcoin::Network;
use express_storage::{managers::checkpoint::CheckpointDbManager, L2BlockManager};
use express_tasks::{TaskExecutor, TaskManager};
use format_serde_error::SerdeError;
use reth_rpc_types::engine::{JwtError, JwtSecret};
use rockbound::OptimisticTransactionDB;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::broadcast};
use tracing::*;

use crate::{
    args::Args,
    config::{Config, SequencerConfig},
    start_status,
};

type CommonDb =
    CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, RBCheckpointDB>;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("config: {0}")]
    MalformedConfig(#[from] SerdeError),

    #[error("jwt: {0}")]
    MalformedSecret(#[from] JwtError),
}

pub fn init_core_dbs(
    rbdb: Arc<OptimisticTransactionDB>,
    db_ops: DbOpsConfig,
) -> (Arc<CommonDb>, Arc<BroadcastDb>) {
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
    let checkpoint_db = Arc::new(alpen_express_rocksdb::RBCheckpointDB::new(
        rbdb.clone(),
        db_ops,
    ));
    let database = Arc::new(alpen_express_db::database::CommonDatabase::new(
        l1_db,
        l2_db,
        sync_ev_db,
        cs_db,
        chst_db,
        checkpoint_db,
    ));
    (database, bcast_db)
}

pub fn initialize_sequencer_database(
    rbdb: Arc<OptimisticTransactionDB>,
    db_ops: DbOpsConfig,
) -> Arc<SequencerDB<SeqDb>> {
    let seqdb = Arc::new(SeqDb::new(rbdb, db_ops));
    Arc::new(SequencerDB::new(seqdb))
}

pub fn get_config(args: Args) -> Result<Config, InitError> {
    match args.config.as_ref() {
        Some(config_path) => {
            // Values passed over arguments get the precedence over the configuration files
            let mut config = load_configuration(config_path)?;
            config.update_from_args(&args);
            Ok(config)
        }
        None => match Config::from_args(&args) {
            Err(msg) => {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }
            Ok(cfg) => Ok(cfg),
        },
    }
}

fn load_configuration(path: &Path) -> Result<Config, InitError> {
    let config_str = fs::read_to_string(path)?;
    let conf = toml::from_str::<Config>(&config_str)
        .map_err(|err| SerdeError::new(config_str.to_string(), err))?;
    Ok(conf)
}

pub fn load_jwtsecret(path: &Path) -> Result<JwtSecret, InitError> {
    let secret = fs::read_to_string(path)?;
    let jwt_secret = JwtSecret::from_hex(secret)?;

    Ok(jwt_secret)
}

pub fn load_rollup_params_or_default(path: &Option<PathBuf>) -> Result<RollupParams, InitError> {
    let Some(path) = path else {
        return Ok(default_rollup_params());
    };
    let json = fs::read_to_string(path)?;
    let rollup_params = serde_json::from_str::<RollupParams>(&json)
        .map_err(|err| SerdeError::new(json.to_string(), err))?;

    Ok(rollup_params)
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
        proof_publish_mode: ProofPublishMode::Strict,
        max_deposits_in_block: 16,
    }
}

pub fn create_bitcoin_rpc(config: &Config) -> anyhow::Result<Arc<BitcoinClient>> {
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
    Ok(btc_rpc)
}

pub fn init_sequencer(
    seq_config: &SequencerConfig,
    config: &Config,
    rpc: Arc<BitcoinClient>,
    task_manager: &TaskManager,
    seq_db: Arc<SequencerDB<SeqDb>>,
    mgr_ctx: &ManagerContext,
    checkpoint_handle: Arc<CheckpointHandle>,
) -> anyhow::Result<Arc<InscriptionHandle>> {
    info!(seqkey_path = ?seq_config.sequencer_key, "initing sequencer duties task");
    let idata = load_seqkey(&seq_config.sequencer_key)?;

    // Set up channel and clone some things.
    let sm = mgr_ctx.sync_manager.clone();
    let cu_rx = sm.create_cstate_subscription();
    let (duties_tx, duties_rx) = broadcast::channel::<DutyBatch>(8);
    let db = mgr_ctx.db.clone();
    let db2 = mgr_ctx.db.clone();
    let eng_ctl_de = mgr_ctx.engine_ctl.clone();
    let pool = mgr_ctx.pool.clone();

    // Spawn up writer
    let writer_config = WriterConfig::new(
        seq_config.sequencer_bitcoin_address.clone(),
        config.bitcoind_rpc.network,
        mgr_ctx.params.rollup().rollup_name.clone(),
    )?;

    let ex = task_manager.executor();
    // Start inscription tasks
    let insc_hndlr = Arc::new(start_inscription_task(
        &ex,
        rpc,
        writer_config,
        seq_db,
        mgr_ctx.status_tx.clone(),
        pool.clone(),
        mgr_ctx.broadcast_handle.clone(),
    )?);

    let ih = insc_hndlr.clone();

    // Spawn duty tasks.
    let t_l2blkman = mgr_ctx.l2block_manager.clone();
    let t_params = mgr_ctx.params.clone();
    ex.spawn_critical("duty_worker::duty_tracker_task", move |shutdown| {
        duty_worker::duty_tracker_task(
            shutdown,
            cu_rx,
            duties_tx,
            idata.ident,
            db,
            t_l2blkman,
            t_params,
        )
        .unwrap()
    });

    let d_params = mgr_ctx.params.clone();
    let d_executor = task_manager.executor();
    ex.spawn_critical("duty_worker::duty_dispatch_task", move |shutdown| {
        duty_worker::duty_dispatch_task(
            shutdown,
            d_executor,
            duties_rx,
            idata.key,
            sm,
            db2,
            eng_ctl_de,
            ih,
            pool,
            d_params,
            checkpoint_handle,
        )
    });
    Ok(insc_hndlr.clone())
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

pub struct ManagerContext {
    db: Arc<CommonDb>,
    pool: threadpool::ThreadPool,
    params: Arc<Params>,
    pub broadcast_handle: Arc<L1BroadcastHandle>,
    pub sync_manager: Arc<SyncManager>,
    pub l2block_manager: Arc<L2BlockManager>,
    pub status_tx: Arc<StatusTx>,
    pub status_rx: Arc<StatusRx>,
    pub engine_ctl: Arc<RpcExecEngineCtl<EngineRpcClient>>,
}

impl ManagerContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<CommonDb>,
        pool: threadpool::ThreadPool,
        params: Arc<Params>,
        broadcast_handle: Arc<L1BroadcastHandle>,
        sync_manager: Arc<SyncManager>,
        l2block_manager: Arc<L2BlockManager>,
        status_tx: Arc<StatusTx>,
        status_rx: Arc<StatusRx>,
        engine_ctl: Arc<RpcExecEngineCtl<EngineRpcClient>>,
    ) -> Self {
        Self {
            db,
            pool,
            params,
            broadcast_handle,
            sync_manager,
            l2block_manager,
            status_tx,
            status_rx,
            engine_ctl,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn init_tasks(
    pool: threadpool::ThreadPool,
    db: Arc<CommonDb>,
    params: Arc<Params>,
    config: &Config,
    rt: &Runtime,
    executor: &TaskExecutor,
    bcast_db: Arc<BroadcastDb>,
    btc_rpc: Arc<BitcoinClient>,
    checkpoint_manager: Arc<CheckpointDbManager>,
) -> anyhow::Result<ManagerContext> {
    let l2block_manager = Arc::new(L2BlockManager::new(pool.clone(), db.clone()));
    let broadcast_handle = init_broadcast_handle(bcast_db, pool.clone(), executor, btc_rpc);

    // init status tasks
    let (status_tx, status_rx) = start_status(db.clone(), params.clone())?;

    let engine_ctl = init_engine_controller(
        config,
        db.clone(),
        params.as_ref(),
        l2block_manager.clone(),
        rt,
    )?;

    // Start the sync manager.
    let sync_manager = Arc::new(sync_manager::start_sync_tasks(
        executor,
        db.clone(),
        l2block_manager.clone(),
        engine_ctl.clone(),
        pool.clone(),
        params.clone(),
        (status_tx.clone(), status_rx.clone()),
        checkpoint_manager,
    )?);

    Ok(ManagerContext {
        params,
        pool,
        db,
        l2block_manager,
        broadcast_handle,
        engine_ctl,
        status_tx,
        status_rx,
        sync_manager,
    })
}

pub fn init_engine_controller(
    config: &Config,
    db: Arc<CommonDb>,
    params: &Params,
    l2block_mgr: Arc<L2BlockManager>,
    rt: &Runtime,
) -> anyhow::Result<Arc<RpcExecEngineCtl<EngineRpcClient>>> {
    let reth_jwtsecret = load_jwtsecret(&config.exec.reth.secret)?;
    let client = EngineRpcClient::from_url_secret(
        &format!("http://{}", &config.exec.reth.rpc_url),
        reth_jwtsecret,
    );

    let initial_fcs = fork_choice_state_initial(db, params.rollup())?;
    let eng_ctl = alpen_express_evmexec::engine::RpcExecEngineCtl::new(
        client,
        initial_fcs,
        rt.handle().clone(),
        l2block_mgr.clone(),
    );
    let eng_ctl = Arc::new(eng_ctl);
    Ok(eng_ctl)
}

fn init_broadcast_handle(
    bcast_db: Arc<BroadcastDb>,
    pool: threadpool::ThreadPool,
    executor: &TaskExecutor,
    btc_rpc: Arc<BitcoinClient>,
) -> Arc<L1BroadcastHandle> {
    // Set up L1 broadcaster.
    let bcastdb = Arc::new(BroadcastDatabase::new(bcast_db));
    let bcast_ctx = express_storage::ops::l1tx_broadcast::Context::new(bcastdb.clone());
    let bcast_ops = Arc::new(bcast_ctx.into_ops(pool.clone()));
    // start broadcast task
    let bcast_handle = spawn_broadcaster_task(executor, btc_rpc.clone(), bcast_ops);
    Arc::new(bcast_handle)
}
