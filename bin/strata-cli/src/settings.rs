use std::{
    fs::{create_dir_all, File},
    path::PathBuf,
    sync::LazyLock,
};

use bdk_wallet::bitcoin::Network;
use config::Config;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub static SETTINGS: LazyLock<Settings> = LazyLock::new(|| {
    let proj_dirs =
        ProjectDirs::from("io", "alpenlabs", "strata").expect("project dir should be available");
    let config_file = proj_dirs.config_dir().to_owned().join("config.toml");
    create_dir_all(config_file.parent().unwrap()).unwrap();
    let _ = File::create_new(&config_file);
    let from_file = Config::builder()
        .add_source(config::File::from(config_file))
        .build()
        .expect("a valid config")
        .try_deserialize::<SettingsFromFile>()
        .expect("a valid config");
    Settings {
        esplora: from_file
            .esplora
            .unwrap_or("https://explorer.bc-2.jp/api".to_owned()),
        l2_http_endpoint: from_file
            .l2_http_endpoint
            .unwrap_or("https://ethereum-rpc.publicnode.com".to_owned()),
        data_dir: proj_dirs.data_dir().to_owned(),
        network: Network::Signet,
        faucet_endpoint: "http://localhost:3000".to_owned(),
    }
});

#[derive(Serialize, Deserialize)]
pub struct SettingsFromFile {
    pub esplora: Option<String>,
    pub l2_http_endpoint: Option<String>,
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
}
