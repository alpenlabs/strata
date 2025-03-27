use std::{
    fs::{create_dir_all, File},
    io,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, LazyLock},
};

use alloy::primitives::Address as AlpenAddress;
use bdk_bitcoind_rpc::bitcoincore_rpc::{Auth, Client};
use bdk_wallet::bitcoin::{Network, XOnlyPublicKey};
use config::Config;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use shrex::{decode, Hex};
use terrors::OneOf;

use crate::{
    constants::{BRIDGE_MUSIG2_PUBKEY, BRIDGE_STRATA_ADDRESS, DEFAULT_NETWORK},
    signet::{backend::SignetBackend, EsploraClient},
};

#[derive(Serialize, Deserialize)]
pub struct SettingsFromFile {
    pub esplora: Option<String>,
    pub bitcoind_rpc_user: Option<String>,
    pub bitcoind_rpc_pw: Option<String>,
    pub bitcoind_rpc_cookie: Option<PathBuf>,
    pub bitcoind_rpc_endpoint: Option<String>,
    pub alpen_endpoint: String,
    pub faucet_endpoint: String,
    pub mempool_endpoint: Option<String>,
    pub blockscout_endpoint: Option<String>,
    pub bridge_pubkey: Option<Hex<[u8; 32]>>,
    pub network: Option<Network>,
}

/// Settings struct filled with either config values or
/// opinionated defaults
#[derive(Debug)]
pub struct Settings {
    pub esplora: Option<String>,
    pub alpen_endpoint: String,
    pub data_dir: PathBuf,
    pub faucet_endpoint: String,
    pub bridge_musig2_pubkey: XOnlyPublicKey,
    pub descriptor_db: PathBuf,
    pub mempool_space_endpoint: Option<String>,
    pub blockscout_endpoint: Option<String>,
    pub bridge_alpen_address: AlpenAddress,
    pub linux_seed_file: PathBuf,
    pub network: Network,
    pub config_file: PathBuf,
    pub signet_backend: Arc<dyn SignetBackend>,
}

pub static PROJ_DIRS: LazyLock<ProjectDirs> = LazyLock::new(|| {
    ProjectDirs::from("io", "alpenlabs", "alpen").expect("project dir should be available")
});

pub static CONFIG_FILE: LazyLock<PathBuf> =
    LazyLock::new(|| match std::env::var("CLI_CONFIG").ok() {
        Some(path) => PathBuf::from_str(&path).expect("valid config path"),
        None => PROJ_DIRS.config_dir().to_owned().join("config.toml"),
    });

impl Settings {
    pub fn load() -> Result<Self, OneOf<(io::Error, config::ConfigError)>> {
        let proj_dirs = &PROJ_DIRS;
        let config_file = CONFIG_FILE.as_path();
        let descriptor_file = proj_dirs.data_dir().to_owned().join("descriptors");
        let linux_seed_file = proj_dirs.data_dir().to_owned().join("seed");

        create_dir_all(proj_dirs.config_dir()).map_err(OneOf::new)?;
        create_dir_all(proj_dirs.data_dir()).map_err(OneOf::new)?;

        // create config file if not exists
        let _ = File::create_new(config_file);
        let from_file: SettingsFromFile = Config::builder()
            .add_source(config::File::from(config_file))
            .build()
            .map_err(OneOf::new)?
            .try_deserialize::<SettingsFromFile>()
            .map_err(OneOf::new)?;

        let sync_backend: Arc<dyn SignetBackend> = match (
            from_file.esplora.clone(),
            from_file.bitcoind_rpc_user,
            from_file.bitcoind_rpc_pw,
            from_file.bitcoind_rpc_cookie,
            from_file.bitcoind_rpc_endpoint,
        ) {
            (Some(url), None, None, None, None) => {
                Arc::new(EsploraClient::new(&url).expect("valid esplora url"))
            }
            (None, Some(user), Some(pw), None, Some(url)) => Arc::new(Arc::new(
                Client::new(&url, Auth::UserPass(user, pw)).expect("valid bitcoin core client"),
            )),
            (None, None, None, Some(cookie_file), Some(url)) => Arc::new(Arc::new(
                Client::new(&url, Auth::CookieFile(cookie_file))
                    .expect("valid bitcoin core client"),
            )),
            _ => panic!("invalid config for signet - configure for esplora or bitcoind"),
        };

        Ok(Settings {
            esplora: from_file.esplora,
            alpen_endpoint: from_file.alpen_endpoint,
            data_dir: proj_dirs.data_dir().to_owned(),
            faucet_endpoint: from_file.faucet_endpoint,
            bridge_musig2_pubkey: XOnlyPublicKey::from_slice(&match from_file.bridge_pubkey {
                Some(key) => key.0,
                None => {
                    let mut buf = [0u8; 32];
                    decode(BRIDGE_MUSIG2_PUBKEY, &mut buf).expect("valid hex");
                    buf
                }
            })
            .expect("valid length"),
            descriptor_db: descriptor_file,
            mempool_space_endpoint: from_file.mempool_endpoint,
            blockscout_endpoint: from_file.blockscout_endpoint,
            bridge_alpen_address: AlpenAddress::from_str(BRIDGE_STRATA_ADDRESS)
                .expect("valid alpen address"),
            linux_seed_file,
            network: from_file.network.unwrap_or(DEFAULT_NETWORK),
            config_file: CONFIG_FILE.clone(),
            signet_backend: sync_backend,
        })
    }
}
