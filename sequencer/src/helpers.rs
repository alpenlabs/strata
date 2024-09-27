use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use alpen_express_btcio::rpc::BitcoinClient;
use alpen_express_consensus_logic::{
    duty::types::{Identity, IdentityData, IdentityKey},
    state_tracker,
};
use alpen_express_db::{database::CommonDatabase, traits::Database};
use alpen_express_evmexec::{engine::RpcExecEngineCtl, fork_choice_state_initial, EngineRpcClient};
use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, ProofPublishMode, RollupParams},
    vk::RollupVerifyingKey,
};
use alpen_express_rocksdb::{
    broadcaster::db::BroadcastDatabase, l2::db::L2Db, sequencer::db::SequencerDB, ChainStateDb,
    ClientStateDb, DbOpsConfig, L1BroadcastDb, L1Db, RBCheckpointDB, RBSeqBlobDb, SyncEventDb,
};
use alpen_express_rpc_types::L1Status;
use alpen_express_state::csm_status::CsmStatus;
use alpen_express_status::{create_status_channel, StatusRx, StatusTx};
use anyhow::Context;
use bitcoin::Network;
use express_storage::L2BlockManager;
use format_serde_error::SerdeError;
use reth_rpc_types::engine::{JwtError, JwtSecret};
use rockbound::{rocksdb, OptimisticTransactionDB};
use thiserror::Error;
use tokio::runtime::Runtime;
use tracing::*;

use crate::{args::Args, config::Config};

pub type CommonDb =
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

pub fn init_core_dbs(rbdb: Arc<OptimisticTransactionDB>, ops_config: DbOpsConfig) -> Arc<CommonDb> {
    // Initialize databases.
    let l1_db: Arc<_> = L1Db::new(rbdb.clone(), ops_config).into();
    let l2_db: Arc<_> = L2Db::new(rbdb.clone(), ops_config).into();
    let sync_ev_db: Arc<_> =
        alpen_express_rocksdb::SyncEventDb::new(rbdb.clone(), ops_config).into();
    let clientstate_db: Arc<_> = ClientStateDb::new(rbdb.clone(), ops_config).into();
    let chainstate_db: Arc<_> = ChainStateDb::new(rbdb.clone(), ops_config).into();
    let checkpoint_db: Arc<_> = RBCheckpointDB::new(rbdb.clone(), ops_config).into();
    let database = CommonDatabase::new(
        l1_db,
        l2_db,
        sync_ev_db,
        clientstate_db,
        chainstate_db,
        checkpoint_db,
    );

    database.into()
}

pub fn init_broadcaster_database(
    rbdb: Arc<OptimisticTransactionDB>,
    ops_config: DbOpsConfig,
) -> Arc<BroadcastDatabase> {
    let l1_broadcast_db = L1BroadcastDb::new(rbdb.clone(), ops_config);
    BroadcastDatabase::new(l1_broadcast_db.into()).into()
}

pub fn init_sequencer_database(
    rbdb: Arc<OptimisticTransactionDB>,
    ops_config: DbOpsConfig,
) -> Arc<SequencerDB<RBSeqBlobDb>> {
    let seqdb = RBSeqBlobDb::new(rbdb, ops_config).into();
    SequencerDB::new(seqdb).into()
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
    let conf =
        toml::from_str::<Config>(&config_str).map_err(|err| SerdeError::new(config_str, err))?;
    Ok(conf)
}

pub fn load_jwtsecret(path: &Path) -> Result<JwtSecret, InitError> {
    let secret = fs::read_to_string(path)?;
    Ok(JwtSecret::from_hex(secret)?)
}

pub fn load_rollup_params_or_default(path: &Option<PathBuf>) -> Result<RollupParams, InitError> {
    let Some(path) = path else {
        return Ok(default_rollup_params());
    };
    let json = fs::read_to_string(path)?;
    let rollup_params =
        serde_json::from_str::<RollupParams>(&json).map_err(|err| SerdeError::new(json, err))?;

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

pub fn create_bitcoin_rpc_client(config: &Config) -> anyhow::Result<Arc<BitcoinClient>> {
    // Set up Bitcoin client RPC.
    let bitcoind_url = format!("http://{}", config.bitcoind_rpc.rpc_url);
    let btc_rpc = BitcoinClient::new(
        bitcoind_url,
        config.bitcoind_rpc.rpc_user.clone(),
        config.bitcoind_rpc.rpc_password.clone(),
    )
    .map_err(anyhow::Error::from)?;

    // TODO remove this
    if config.bitcoind_rpc.network != Network::Regtest {
        warn!("network not set to regtest, ignoring");
    }
    Ok(btc_rpc.into())
}

pub fn open_rocksdb_database(
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

pub fn load_seqkey(path: &PathBuf) -> anyhow::Result<IdentityData> {
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

// initializes the status bundle that we can pass around cheaply for status/metrics
pub fn init_status_channel<D>(database: &D) -> anyhow::Result<(Arc<StatusTx>, Arc<StatusRx>)>
where
    D: Database + Send + Sync + 'static,
{
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

pub fn init_engine_controller(
    config: &Config,
    db: Arc<CommonDb>,
    params: &Params,
    l2_block_manager: Arc<L2BlockManager>,
    runtime: &Runtime,
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
        runtime.handle().clone(),
        l2_block_manager.clone(),
    );
    let eng_ctl = Arc::new(eng_ctl);
    Ok(eng_ctl)
}
