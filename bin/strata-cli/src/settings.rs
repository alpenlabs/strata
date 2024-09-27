use std::{
    fs::{create_dir_all, File},
    io,
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use alloy::primitives::Address as RollupAddress;
use bdk_wallet::bitcoin::{Network, XOnlyPublicKey};
use config::Config;
use directories::ProjectDirs;
use hex::decode;
use serde::{Deserialize, Serialize};
use terrors::OneOf;

#[derive(Serialize, Deserialize)]
pub struct SettingsFromFile {
    pub esplora: Option<String>,
    pub l2_http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
/// Settings struct filled with either config values or
/// opinionated defaults
pub struct Settings {
    pub esplora: String,
    pub l2_http_endpoint: String,
    pub data_dir: PathBuf,
    pub network: Network,
    pub faucet_endpoint: String,
    pub bridge_musig2_pubkey: XOnlyPublicKey,
    pub block_time: Duration,
    pub descriptor_db: PathBuf,
    pub bridge_rollup_address: RollupAddress,
    pub linux_seed_file: PathBuf,
}

impl Settings {
    pub fn load() -> Result<Self, OneOf<(io::Error, config::ConfigError)>> {
        let proj_dirs = ProjectDirs::from("io", "alpenlabs", "strata")
            .expect("project dir should be available");
        let config_file = proj_dirs.config_dir().to_owned().join("config.toml");
        let descriptor_file = proj_dirs.data_dir().to_owned().join("descriptors");
        let linux_seed_file = proj_dirs.data_dir().to_owned().join("seed");
        create_dir_all(proj_dirs.config_dir()).map_err(OneOf::new)?;
        create_dir_all(proj_dirs.data_dir()).map_err(OneOf::new)?;
        let _ = File::create_new(&config_file);
        let from_file = Config::builder()
            .add_source(config::File::from(config_file))
            .build()
            .map_err(OneOf::new)?
            .try_deserialize::<SettingsFromFile>()
            .map_err(OneOf::new)?;
        Ok(Settings {
            esplora: from_file
                .esplora
                .unwrap_or("https://explorer.bc-2.jp/api".to_owned()),
            l2_http_endpoint: from_file
                .l2_http_endpoint
                .unwrap_or("https://ethereum-rpc.publicnode.com".to_owned()),
            data_dir: proj_dirs.data_dir().to_owned(),
            network: Network::Signet,
            faucet_endpoint: from_file
                .faucet_endpoint
                .unwrap_or("http://localhost:3000".to_owned()),
            bridge_musig2_pubkey: XOnlyPublicKey::from_slice(&{
                let mut buf = [0u8; 32];
                decode(
                    // just random 32 bytes while we don't have this
                    // CHANGE ME!!!
                    "fbd79b6b8b7fe11bad25ae89a7415221c030978de448775729c3f0a903819dd0",
                    &mut buf,
                )
                .expect("valid hex");
                buf
            })
            .expect("valid length"),
            block_time: Duration::from_secs(30),
            descriptor_db: descriptor_file,
            bridge_rollup_address: RollupAddress::from_str(
                "0x000000000000000000000000000000000B121d9E",
            )
            .expect("valid rollup address"),
            linux_seed_file,
        })
    }
}
