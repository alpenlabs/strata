use std::path::PathBuf;

use argh::FromArgs;
use strata_config::{ClientMode, Config, FullNodeConfig, SequencerConfig};

const JWT_SECRET_ENV_VAR: &str = "JWT_SECRET_ENV_VAR";

/// Configs overriddable by environment. Mostly for sensitive data.
#[derive(Debug, Clone)]
pub struct EnvArgs {
    // TODO: add other relevant items, even jwt is a path in the config.
    jwt_secret: Option<String>,
}

impl EnvArgs {
    pub fn from_env() -> Self {
        Self {
            jwt_secret: std::env::var(JWT_SECRET_ENV_VAR).ok(),
        }
    }

    /// Override some of the config params from env.
    pub fn override_config(&self, config: &mut Config) -> bool {
        let mut overridden = false;

        if let Some(secret) = &self.jwt_secret {
            overridden = true;
            config.exec.reth.secret = secret.into();
        }

        overridden
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

    #[argh(option, description = "is sequencer", default = "false")]
    pub sequencer: bool,

    #[argh(option, description = "sequencer rpc")]
    pub sequencer_rpc: Option<String>,

    #[argh(option, short = 'r', description = "JSON-RPC port")]
    pub rpc_port: Option<u16>,

    #[argh(option, short = 'p', description = "P2P port")]
    pub p2p_port: Option<u16>,

    #[argh(option, description = "rollup params")]
    pub rollup_params: Option<PathBuf>,
}

impl Args {
    /// Override common config params from arg.
    pub fn override_config(&self, config: &mut Config) -> bool {
        let args = self.clone();
        let mut overridden = false;

        if let Some(rpc_port) = args.rpc_port {
            overridden = true;
            config.client.rpc_port = rpc_port;
        }

        if let Some(p2p_port) = args.p2p_port {
            overridden = true;
            config.client.p2p_port = p2p_port;
        }

        if let Some(datadir) = args.datadir {
            overridden = true;
            config.client.datadir = datadir;
        }

        if args.sequencer {
            config.client.client_mode = ClientMode::Sequencer(SequencerConfig {});
        } else if let Some(sequencer_rpc) = args.sequencer_rpc {
            overridden = true;
            config.client.client_mode = ClientMode::FullNode(FullNodeConfig { sequencer_rpc });
        }
        overridden
    }
}
