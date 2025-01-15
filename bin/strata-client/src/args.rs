use std::path::PathBuf;

use argh::FromArgs;
use bitcoin::Network;
use strata_config::{
    bridge::RelayerConfig, btcio::BtcioConfig, BitcoindConfig, ClientConfig, ClientMode, Config,
    ExecConfig, FullNodeConfig, RethELConfig, SequencerConfig, SyncConfig,
};

#[derive(Debug, Clone, FromArgs)]
#[argh(description = "Alpen Strata sequencer")]
pub struct Args {
    // TODO: default config location
    #[argh(option, short = 'c', description = "path to configuration")]
    pub config: Option<PathBuf>,

    #[argh(
        option,
        short = 'd',
        description = "datadir path that will contain databases"
    )]
    pub datadir: Option<PathBuf>,

    #[argh(option, short = 'h', description = "JSON-RPC host")]
    pub rpc_host: Option<String>,

    #[argh(option, short = 'r', description = "JSON-RPC port")]
    pub rpc_port: Option<u16>,

    #[argh(option, description = "bitcoind RPC host")]
    pub bitcoind_host: Option<String>,

    #[argh(option, description = "bitcoind RPC user")]
    pub bitcoind_user: Option<String>,

    #[argh(option, description = "bitcoind RPC password")]
    pub bitcoind_password: Option<String>,

    #[argh(option, short = 'n', description = "L1 network to run on")]
    pub network: Option<Network>,

    #[argh(option, short = 'k', description = "path to sequencer root key")]
    pub sequencer_key: Option<PathBuf>,

    #[argh(option, description = "sequencer rpc host:port")]
    pub sequencer_rpc: Option<String>,

    #[argh(option, description = "reth authrpc host:port")]
    pub reth_authrpc: Option<String>,

    #[argh(option, description = "path to reth authrpc jwtsecret")]
    pub reth_jwtsecret: PathBuf,

    #[argh(option, short = 's', description = "sequencer bitcoin address")]
    pub sequencer_bitcoin_address: Option<String>,

    // TODO: allow only for dev/test mode ?
    #[argh(option, short = 'p', description = "custom rollup config path")]
    pub rollup_params: Option<PathBuf>,

    #[argh(option, description = "database retry count")]
    pub db_retry_count: Option<u16>,
}

impl Args {
    pub fn derive_config(&self) -> Result<Config, String> {
        let args = self.clone();
        Ok(Config {
            bitcoind_rpc: BitcoindConfig {
                rpc_url: require(args.bitcoind_host, "args: no bitcoin --rpc-url provided")?,
                rpc_user: require(args.bitcoind_user, "args: no bitcoin --rpc-user provided")?,
                rpc_password: require(
                    args.bitcoind_password,
                    "args: no bitcoin --rpc-password provided",
                )?,
                network: require(args.network, "args: no bitcoin --network provided")?,
            },
            client: ClientConfig {
                rpc_host: require(args.rpc_host, "args: no client --rpc-host provided")?,
                rpc_port: require(args.rpc_port, "args: no client --rpc-port provided")?,
                datadir: require(args.datadir, "args: no client --datadir provided")?,
                client_mode: {
                    if let Some(sequencer_key) = args.sequencer_key {
                        ClientMode::Sequencer(SequencerConfig {
                            sequencer_key,
                            sequencer_bitcoin_address: args.sequencer_bitcoin_address,
                        })
                    } else if let Some(sequencer_rpc) = args.sequencer_rpc {
                        ClientMode::FullNode(FullNodeConfig { sequencer_rpc })
                    } else {
                        return Err(
                            "args: no client --sequencer-key or --sequencer-bitcion-address provided or --sequencer-rpc provided"
                                .to_string(),
                        );
                    }
                },
                l2_blocks_fetch_limit: 1_000,
                db_retry_count: 5,
            },
            sync: SyncConfig {
                l1_follow_distance: 6,
                client_checkpoint_interval: 10,
            },
            exec: ExecConfig {
                reth: RethELConfig {
                    rpc_url: args.reth_authrpc.unwrap_or("".to_string()), // TODO: sensible default
                    secret: args.reth_jwtsecret,
                },
            },
            relayer: RelayerConfig {
                // TODO: actually get these from args
                refresh_interval: 10,
                stale_duration: 120,
                relay_misc: true,
            },
            btcio: BtcioConfig::default(), // TODO: actually get this from args
        })
    }

    pub fn update_config(&self, config: &mut Config) {
        let args = self.clone();
        config.exec.reth.secret = args.reth_jwtsecret;

        if let Some(rpc_user) = args.bitcoind_user {
            config.bitcoind_rpc.rpc_user = rpc_user;
        }
        if let Some(rpc_url) = args.bitcoind_host {
            config.bitcoind_rpc.rpc_url = rpc_url;
        }
        if let Some(rpc_password) = args.bitcoind_password {
            config.bitcoind_rpc.rpc_password = rpc_password;
        }
        if let Some(rpc_host) = args.rpc_host {
            config.client.rpc_host = rpc_host;
        }
        if let Some(rpc_port) = args.rpc_port {
            config.client.rpc_port = rpc_port;
        }
        if let Some(datadir) = args.datadir {
            config.client.datadir = datadir;
        }
        // sequencer_key has priority over sequencer_rpc if both are provided

        if let Some(sequencer_key) = args.sequencer_key {
            config.client.client_mode = ClientMode::Sequencer(SequencerConfig {
                sequencer_key,
                sequencer_bitcoin_address: args.sequencer_bitcoin_address,
            });
        } else if let Some(sequencer_rpc) = args.sequencer_rpc {
            config.client.client_mode = ClientMode::FullNode(FullNodeConfig { sequencer_rpc });
        }
        if let Some(rpc_url) = args.reth_authrpc {
            config.exec.reth.rpc_url = rpc_url;
        }
        if let Some(db_retry_count) = args.db_retry_count {
            config.client.db_retry_count = db_retry_count;
        }
    }
}

fn require<T>(field: Option<T>, err_msg: &str) -> Result<T, String> {
    field.ok_or_else(|| err_msg.to_string())
}
