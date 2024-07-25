use std::path::PathBuf;

use bitcoin::Network;
use serde::Deserialize;

use crate::args::Args;

#[derive(Deserialize, Debug)]
pub struct ClientParams {
    pub rpc_port: u16,
    pub sequencer_key: Option<PathBuf>,

    /// The address to which the inscriptions are spent
    pub sequencer_bitcoin_address: String, // TODO: probably move this to another struct
    pub datadir: PathBuf,
}

#[derive(Deserialize, Debug)]
pub struct SyncParams {
    pub l1_follow_distance: u64,
    pub max_reorg_depth: u32,
    pub client_poll_dur_ms: u32,
}

#[derive(Deserialize, Debug)]
pub struct BitcoindParams {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
}

#[derive(Deserialize, Debug)]
pub struct RethELParams {
    pub rpc_url: String,
    pub secret: PathBuf,
}

#[derive(Deserialize, Debug)]
pub struct ExecParams {
    pub reth: RethELParams,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub client: ClientParams,
    pub bitcoind_rpc: BitcoindParams,
    pub sync: SyncParams,
    pub exec: ExecParams,
}

impl Config {
    pub fn from_args(args: &Args) -> Config {
        // TODO: get everything from args or from toml
        Self {
            bitcoind_rpc: BitcoindParams {
                rpc_url: args.bitcoind_host.clone(),
                rpc_user: args.bitcoind_user.clone(),
                rpc_password: args.bitcoind_password.clone(),
                network: Network::from_core_arg(&args.network)
                    .expect("required valid bitcoin network"),
            },
            client: ClientParams {
                rpc_port: args.rpc_port,
                datadir: args.datadir.clone(),
                sequencer_key: None,
                sequencer_bitcoin_address: args.sequencer_bitcoin_address.clone(),
            },
            sync: SyncParams {
                l1_follow_distance: 6,
                max_reorg_depth: 4,
                client_poll_dur_ms: 200,
            },
            exec: ExecParams {
                reth: RethELParams {
                    rpc_url: String::new(),
                    secret: PathBuf::new(),
                },
            },
        }
    }
    pub fn update_from_args(&mut self, args: &Args) {
        let args = args.clone();

        if let Some(rpc_user) = args.bitcoind_user {
            self.bitcoind_rpc.rpc_user = rpc_user;
        }
        if let Some(rpc_url) = args.bitcoind_host {
            self.bitcoind_rpc.rpc_url = rpc_url;
        }
        if let Some(rpc_password) = args.bitcoind_password {
            self.bitcoind_rpc.rpc_password = rpc_password;
        }
        if let Some(rpc_port) = args.rpc_port {
            self.client.rpc_port = rpc_port;
        }
        if let Some(datadir) = args.datadir {
            self.client.datadir = datadir;
        }
        if args.sequencer_key.is_some() {
            self.client.sequencer_key = args.sequencer_key;
        }
        if let Some(rpc_url) = args.reth_authrpc {
            self.exec.reth.rpc_url = rpc_url;
        }
        if let Some(jwtsecret) = args.reth_jwtsecret {
            self.exec.reth.secret = jwtsecret;
        }
        if let Some(seq_addr) = args.sequencer_bitcoin_address {
            self.client.sequencer_bitcoin_address = seq_addr;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::config::Config;

    #[test]
    fn config_load_test() {
        let config_string = r#"
            [bitcoind_rpc]
            rpc_url = "http://localhost:18332"
            rpc_user = "alpen"
            rpc_password = "alpen"
            network = "regtest"

            [client]
            rpc_port = 8432
            datadir = "/path/to/data/directory"
            sequencer_bitcoin_address = "some_addr"

            [sync]
            l1_follow_distance = 6
            max_reorg_depth = 4
            client_poll_dur_ms = 200

            [exec.reth]
            rpc_url = "http://localhost:8551"
            secret = "1234567890abcdef"
        "#;

        assert!(toml::from_str::<Config>(config_string).is_ok());
    }
}
