use std::{
    fs::{create_dir_all, File},
    io,
    path::PathBuf,
    str::FromStr,
};

use alloy::primitives::Address as StrataAddress;
use bdk_wallet::bitcoin::XOnlyPublicKey;
use config::Config;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use shrex::decode;
use terrors::OneOf;

use crate::constants::{
    BRIDGE_MUSIG2_PUBKEY, BRIDGE_STRATA_ADDRESS, DEFAULT_ESPLORA, DEFAULT_FAUCET_ENDPOINT,
    DEFAULT_L2_HTTP_ENDPOINT,
};

#[derive(Serialize, Deserialize)]
pub struct SettingsFromFile {
    pub esplora: Option<String>,
    pub l2_http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
}

/// Settings struct filled with either config values or
/// opinionated defaults
#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub esplora: String,
    pub l2_http_endpoint: String,
    pub data_dir: PathBuf,
    pub faucet_endpoint: String,
    pub bridge_musig2_pubkey: XOnlyPublicKey,
    pub descriptor_db: PathBuf,
    pub bridge_strata_address: StrataAddress,
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
            esplora: from_file.esplora.unwrap_or(DEFAULT_ESPLORA.to_owned()),
            l2_http_endpoint: from_file
                .l2_http_endpoint
                .unwrap_or(DEFAULT_L2_HTTP_ENDPOINT.to_owned()),
            data_dir: proj_dirs.data_dir().to_owned(),
            faucet_endpoint: from_file
                .faucet_endpoint
                .unwrap_or(DEFAULT_FAUCET_ENDPOINT.to_owned()),
            bridge_musig2_pubkey: XOnlyPublicKey::from_slice(&{
                let mut buf = [0u8; 32];
                decode(BRIDGE_MUSIG2_PUBKEY, &mut buf).expect("valid hex");
                buf
            })
            .expect("valid length"),
            descriptor_db: descriptor_file,
            bridge_strata_address: StrataAddress::from_str(BRIDGE_STRATA_ADDRESS)
                .expect("valid strata address"),
            linux_seed_file,
        })
    }
}
