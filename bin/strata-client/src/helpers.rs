use std::{fs, path::Path, sync::Arc, time::Duration};

use alloy_rpc_types::engine::JwtSecret;
use bitcoin::{base58, bip32::Xpriv, Address, Network};
use format_serde_error::SerdeError;
use strata_btcio::rpc::{traits::Wallet, BitcoinClient};
use strata_config::Config;
use strata_consensus_logic::{
    csm::state_tracker,
    duty::types::{Identity, IdentityData, IdentityKey},
};
use strata_db::traits::Database;
use strata_evmexec::{engine::RpcExecEngineCtl, fork_choice_state_initial, EngineRpcClient};
use strata_key_derivation::sequencer::SequencerKeys;
use strata_primitives::{
    buf::Buf32,
    keys::ZeroizableXpriv,
    l1::L1Status,
    params::{Params, RollupParams, SyncParams},
};
use strata_rocksdb::CommonDb;
use strata_state::csm_status::CsmStatus;
use strata_status::StatusChannel;
use strata_storage::L2BlockManager;
use tokio::runtime::Handle;
use tracing::*;
use zeroize::Zeroize;

use crate::{args::Args, errors::InitError, network};

pub fn get_config(args: Args) -> Result<Config, InitError> {
    match args.config.as_ref() {
        Some(config_path) => {
            // Values passed over arguments get the precedence over the configuration files
            let mut config = load_configuration(config_path)?;
            args.update_config(&mut config);
            Ok(config)
        }
        None => match args.derive_config() {
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
pub fn resolve_and_validate_params(
    path: Option<&Path>,
    config: &Config,
) -> Result<Arc<Params>, InitError> {
    let rollup_params = resolve_rollup_params(path)?;
    rollup_params.check_well_formed()?;

    let params = Params {
        rollup: rollup_params,
        run: SyncParams {
            // FIXME these shouldn't be configurable here
            l1_follow_distance: config.sync.l1_follow_distance,
            client_checkpoint_interval: config.sync.client_checkpoint_interval,
            l2_blocks_fetch_limit: config.client.l2_blocks_fetch_limit,
        },
    }
    .into();
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

// TODO: remove this after builder is done
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

/// Loads sequencer identity data from the root key at the specified path.
pub fn load_seqkey(path: &Path) -> anyhow::Result<IdentityData> {
    let str_buf = fs::read_to_string(path)?;
    let str_buf = str_buf.trim();
    debug!(?path, "loading sequencer root key");
    let buf = base58::decode_check(str_buf)?;
    let master_xpriv = ZeroizableXpriv::new(Xpriv::decode(&buf)?);

    // Actually do the key derivation from the root key and then derive the pubkey from that.
    let seq_keys = SequencerKeys::new(&master_xpriv)?;
    let seq_xpriv = seq_keys.derived_xpriv();
    let mut seq_sk = Buf32::from(seq_xpriv.private_key.secret_bytes());
    let seq_xpub = seq_keys.derived_xpub();
    let seq_pk = seq_xpub.to_x_only_pub().serialize();

    let ik = IdentityKey::Sequencer(seq_sk);
    let ident = Identity::Sequencer(Buf32::from(seq_pk));

    // Zeroize the Buf32 representation of the Xpriv.
    seq_sk.zeroize();

    // Changed this to the pubkey so that we don't just log our privkey.
    debug!(?ident, "ready to sign as sequencer");

    let idata = IdentityData::new(ident, ik);
    Ok(idata)
}

// initializes the status bundle that we can pass around cheaply for status/metrics
pub fn init_status_channel<D>(database: &D) -> anyhow::Result<StatusChannel>
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
        ..Default::default()
    };

    Ok(StatusChannel::new(cur_state, l1_status, None))
}

pub fn init_engine_controller(
    config: &Config,
    db: Arc<CommonDb>,
    params: &Params,
    l2_block_manager: Arc<L2BlockManager>,
    handle: &Handle,
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
        handle.clone(),
        l2_block_manager,
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
