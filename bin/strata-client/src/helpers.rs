use std::{fs, path::Path, sync::Arc, time::Duration};

use anyhow::Context;
use bitcoin::{
    base58,
    bip32::{Xpriv, Xpub},
    secp256k1::SECP256K1,
    Address, Network,
};
use format_serde_error::SerdeError;
use reth_rpc_types::engine::JwtSecret;
use rockbound::{rocksdb, OptimisticTransactionDB};
use strata_btcio::rpc::{traits::Wallet, BitcoinClient};
use strata_consensus_logic::{
    csm::state_tracker,
    duty::types::{Identity, IdentityData, IdentityKey},
};
use strata_db::{database::CommonDatabase, traits::Database};
use strata_evmexec::{engine::RpcExecEngineCtl, fork_choice_state_initial, EngineRpcClient};
use strata_primitives::{
    buf::Buf32,
    params::{Params, RollupParams},
};
use strata_rocksdb::{
    broadcaster::db::BroadcastDb, l2::db::L2Db, sequencer::db::SequencerDB, ChainstateDb,
    ClientStateDb, DbOpsConfig, L1BroadcastDb, L1Db, RBCheckpointDB, RBSeqBlobDb, SyncEventDb,
};
use strata_rpc_types::L1Status;
use strata_state::csm_status::CsmStatus;
use strata_status::{create_status_channel, StatusRx, StatusTx};
use strata_storage::L2BlockManager;
use tokio::runtime::Runtime;
use tracing::*;

use crate::{args::Args, config::Config, errors::InitError, keyderiv, network};

pub type CommonDb =
    CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainstateDb, RBCheckpointDB>;

pub fn init_core_dbs(rbdb: Arc<OptimisticTransactionDB>, ops_config: DbOpsConfig) -> Arc<CommonDb> {
    // Initialize databases.
    let l1_db: Arc<_> = L1Db::new(rbdb.clone(), ops_config).into();
    let l2_db: Arc<_> = L2Db::new(rbdb.clone(), ops_config).into();
    let sync_ev_db: Arc<_> = strata_rocksdb::SyncEventDb::new(rbdb.clone(), ops_config).into();
    let clientstate_db: Arc<_> = ClientStateDb::new(rbdb.clone(), ops_config).into();
    let chainstate_db: Arc<_> = ChainstateDb::new(rbdb.clone(), ops_config).into();
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
) -> Arc<BroadcastDb> {
    let l1_broadcast_db = L1BroadcastDb::new(rbdb.clone(), ops_config);
    BroadcastDb::new(l1_broadcast_db.into()).into()
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

/// Resolves the rollup params file to use, possibly from a path, and validates
/// it to ensure it passes sanity checks.
pub fn resolve_and_validate_rollup_params(path: Option<&Path>) -> Result<RollupParams, InitError> {
    let params = resolve_rollup_params(path)?;
    params.check_well_formed()?;
    Ok(params)
}

/// Resolves the rollup params file to use, possibly from a path.
pub fn resolve_rollup_params(path: Option<&Path>) -> Result<RollupParams, InitError> {
    // If a path is set from arg load that.
    if let Some(p) = path {
        return load_rollup_params(p);
    }

    // Otherwise check from envvar.
    if let Some(p) = network::get_envvar_params()? {
        return Ok(p);
    }

    // *Otherwise*, use the fallback.
    Ok(network::get_default_rollup_params()?)
}

fn load_rollup_params(path: &Path) -> Result<RollupParams, InitError> {
    let json = fs::read_to_string(path)?;
    let rollup_params =
        serde_json::from_str::<RollupParams>(&json).map_err(|err| SerdeError::new(json, err))?;
    Ok(rollup_params)
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

    let dbname = strata_rocksdb::ROCKSDB_NAME;
    let cfs = strata_rocksdb::STORE_COLUMN_FAMILIES;
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

/// Loads sequencer identity data from the root key at the specified path.
pub fn load_seqkey(path: &Path) -> anyhow::Result<IdentityData> {
    let raw_buf = fs::read(path)?;
    let str_buf = std::str::from_utf8(&raw_buf)?;
    debug!(?path, "loading sequencer root key");
    let buf = base58::decode_check(str_buf)?;
    let root_xpriv = Xpriv::decode(&buf)?;

    // Actually do the key derivation from the root key and then derive the pubkey from that.
    let seq_xpriv = keyderiv::derive_seq_xpriv(&root_xpriv)?;
    let seq_sk = Buf32::from(seq_xpriv.private_key.secret_bytes());
    let seq_xpub = Xpub::from_priv(SECP256K1, &seq_xpriv);
    let seq_pk = seq_xpub.to_x_only_pub().serialize();

    let ik = IdentityKey::Sequencer(seq_sk);
    let ident = Identity::Sequencer(Buf32::from(seq_pk));

    // Changed this to the pubkey so that we don't just log our privkey.
    debug!(?ident, "ready to sign as sequencer");

    let idata = IdentityData::new(ident, ik);
    Ok(idata)
}

// initializes the status bundle that we can pass around cheaply for status/metrics
pub fn init_status_channel<D>(
    database: &D,
    network: Network,
) -> anyhow::Result<(Arc<StatusTx>, Arc<StatusRx>)>
where
    D: Database + Send + Sync + 'static,
{
    // init client state
    let cs_db = database.client_state_db().as_ref();
    let (cur_state_idx, cur_state) = state_tracker::reconstruct_cur_state(cs_db)?;

    // init the CsmStatus
    let mut status = CsmStatus::default();
    status.set_last_sync_ev_idx(cur_state_idx);
    status.update_from_client_state(&cur_state);

    let l1_status = L1Status {
        network,
        ..Default::default()
    };

    Ok(create_status_channel(status, cur_state, l1_status))
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
    let eng_ctl = strata_evmexec::engine::RpcExecEngineCtl::new(
        client,
        initial_fcs,
        runtime.handle().clone(),
        l2_block_manager.clone(),
    );
    let eng_ctl = Arc::new(eng_ctl);
    Ok(eng_ctl)
}

/// Get an address controlled by sequencer's bitcoin wallet
pub async fn generate_sequencer_address(
    bitcoin_client: &BitcoinClient,
    timeout: u64,
    poll_interval: u64,
) -> anyhow::Result<Address> {
    let mut last_err = None;
    tokio::time::timeout(Duration::from_secs(timeout), async {
        loop {
            match bitcoin_client.get_new_address().await {
                Ok(address) => return address,
                Err(err) => {
                    warn!(err = ?err, "failed to generate address");
                    last_err.replace(err);
                }
            }
            // Sleep for a while just to prevent excessive continuous calls in short time
            tokio::time::sleep(Duration::from_millis(poll_interval)).await;
        }
    })
    .await
    .map_err(|_| match last_err {
        None => anyhow::Error::msg("failed to generate address; timeout"),
        Some(client_error) => {
            anyhow::Error::from(client_error).context("failed to generate address")
        }
    })
}
