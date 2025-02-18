use std::path::PathBuf;

use anyhow::anyhow;
use argh::FromArgs;
use serde::de::DeserializeOwned;
use serde_json::{from_value, to_value, Value};
use strata_config::Config;

/// Configs overridable by environment. Mostly for sensitive data.
#[derive(Debug, Clone)]
pub struct EnvArgs {
    // TODO: relevant items that will be populated from env vars
}

impl EnvArgs {
    pub fn from_env() -> Self {
        // Here we load particular env vars that should probably override the config.
        Self {}
    }

    /// Override some of the config params from env.
    pub fn override_config(&self, _config: &mut Config) -> bool {
        // Override attributes
        true
    }
}

#[derive(Debug, Clone, FromArgs)]
#[argh(description = "Strata client")]
pub struct Args {
    // Config non-overriding args
    #[argh(option, short = 'c', description = "path to configuration")]
    pub config: PathBuf,

    // Config overriding args
    #[argh(
        option,
        short = 'd',
        description = "datadir path that will contain databases"
    )]
    pub datadir: Option<PathBuf>,

    #[argh(switch, description = "is sequencer")]
    pub sequencer: bool,

    #[argh(option, description = "rollup params")]
    pub rollup_params: Option<PathBuf>,

    #[argh(option, description = "rpc host")]
    pub rpc_host: Option<String>,

    #[argh(option, description = "rpc port")]
    pub rpc_port: Option<u16>,

    #[argh(option, short = 'o', description = "generic config overrides")]
    pub overrides: Vec<String>,
}

impl Args {
    /// Overrides config. First overrides with the generic overrides passed via -o and then
    /// overrides the result with a couple of commonly passed args.
    pub fn override_config(&self, config: &mut Config) -> anyhow::Result<bool> {
        // Override using -o params.
        let mut overridden = self.override_generic(config)?;

        // Override by explicitly parsed args like datadir, rpc_host and rpc_port.
        if let Some(datadir) = &self.datadir {
            config.client.datadir = datadir.into();
            overridden = true
        }
        if let Some(rpc_host) = &self.rpc_host {
            config.client.rpc_host = rpc_host.to_string();
            overridden = true
        }
        if let Some(rpc_port) = &self.rpc_port {
            config.client.rpc_port = *rpc_port;
            overridden = true
        }
        Ok(overridden)
    }

    /// Override config using the generic overrides. The idea here is to first convert the
    /// [`Config`] to json and then apply the overrides by splitting the key by a dot.
    fn override_generic(&self, config: &mut Config) -> anyhow::Result<bool> {
        let original = config.clone();
        // Convert config as json
        let mut json_config = to_value(&mut *config).expect("Config json serialization failed");

        for (path, val) in parse_overrides(&self.overrides)?.iter() {
            apply_override(path, val, &mut json_config)?;
        }
        // Convert back to json.
        *config =
            from_value(json_config).expect("Should be able to create Config from serde json Value");
        Ok(original != *config)
    }
}

type Override = (Vec<String>, String);

/// Parse valid overrides. This first splits the entries by '=' to get key and value and then splits
/// the key by '.' which is the update path.
fn parse_overrides(overrides: &[String]) -> anyhow::Result<Vec<Override>> {
    let mut result = Vec::new();
    for item in overrides {
        let (key, value) = item
            .split_once("=")
            .ok_or(anyhow!("Invalid override: must be in 'key=value' format"))?;
        let path: Vec<_> = key.split(".").map(|x| x.to_string()).collect();
        result.push((path, value.to_string()));
    }
    Ok(result)
}

/// Apply override to config.
fn apply_override(path: &[String], str_value: &str, config: &mut Value) -> anyhow::Result<()> {
    match path {
        [key] => {
            config[key] = parse_value(str_value)?;
        }
        [key, other @ ..] => {
            apply_override(other, str_value, &mut config[key])?;
        }
        [] => return Err(anyhow!("Invalid override path")),
    };
    Ok(())
}

/// Parses a string `T`. If parsing fails, attempts to parse it again by converting it to JSON
/// string i.e. surrounds the string with double quotes.
/// If the second deserialization attempt also fails, it returns the error from the first attempt.
fn parse_value<T: DeserializeOwned>(str_value: &str) -> Result<T, serde_json::Error> {
    match serde_json::from_str(str_value) {
        Ok(v) => Ok(v),
        Err(e) => match serde_json::from_str::<T>(&format!("\"{str_value}\"")) {
            Ok(v) => Ok(v),
            Err(_) => Err(e),
        },
    }
}

#[cfg(test)]
mod test {

    use bitcoin::Network;
    use strata_config::ClientConfig;

    use super::*;

    fn get_config() -> Config {
        Config {
            client: ClientConfig {
                rpc_host: "".to_string(),
                rpc_port: 300,
                p2p_port: 300,
                sync_endpoint: None,
                l2_blocks_fetch_limit: 20,
                datadir: "".into(),
                db_retry_count: 3,
            },
            bitcoind: strata_config::BitcoindConfig {
                rpc_url: "".to_string(),
                rpc_user: "".to_string(),
                rpc_password: "".to_string(),
                network: bitcoin::Network::Regtest,
                retry_count: None,
                retry_interval: None,
            },
            btcio: strata_config::btcio::BtcioConfig {
                reader: Default::default(),
                writer: Default::default(),
                broadcaster: Default::default(),
            },
            exec: strata_config::ExecConfig {
                reth: strata_config::RethELConfig {
                    rpc_url: "".to_string(),
                    secret: "".into(),
                },
            },
            relayer: strata_config::bridge::RelayerConfig {
                refresh_interval: 1,
                stale_duration: 2,
                relay_misc: false,
            },
            sync: strata_config::SyncConfig {
                l1_follow_distance: 1,
                client_checkpoint_interval: 2,
            },
        }
    }

    #[test]
    fn test_generic_override() {
        let mut config = get_config();
        let args = Args {
            config: "config_path".into(),
            datadir: None,
            sequencer: false,
            rollup_params: None,
            rpc_host: None,
            rpc_port: None,
            overrides: vec![
                "btcio.reader.client_poll_dur_ms=50".to_string(),
                "sync.l1_follow_distance=30".to_string(),
                "client.rpc_host=rpchost".to_string(),
                "bitcoind.network=signet".to_string(),
            ],
        };
        // First assert config doesn't already have the expected values after overriding
        assert!(config.btcio.reader.client_poll_dur_ms != 50);
        assert!(config.sync.l1_follow_distance != 30);

        let res = args.override_config(&mut config);
        println!("override result: {:?}", res);
        assert!(res.is_ok());

        assert!(config.btcio.reader.client_poll_dur_ms == 50);
        assert!(config.sync.l1_follow_distance == 30);
        assert!(&config.client.rpc_host == "rpchost");
        assert!(config.bitcoind.network == Network::Signet);
    }
}
