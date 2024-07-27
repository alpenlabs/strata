use std::path::PathBuf;

use bitcoin::Network;
use serde::Deserialize;

use crate::args::Args;

#[derive(Deserialize, Debug)]
pub struct ClientParams {
    pub rpc_port: u16,
    pub sequencer_key: Option<PathBuf>,
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
    pub fn new() -> Config {
        Self {
            bitcoind_rpc: BitcoindParams {
                rpc_url: String::new(),
                rpc_user: String::new(),
                rpc_password: String::new(),
                network: Network::Regtest,
            },
            client: ClientParams {
                rpc_port: 8432,
                datadir: PathBuf::new(),
                sequencer_key: None,
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
        self.bitcoind_rpc.rpc_user = args.bitcoind_user;
        self.bitcoind_rpc.rpc_url = args.bitcoind_host;
        self.client.rpc_port = args.rpc_port;
        self.bitcoind_rpc.rpc_password = args.bitcoind_password;
        self.client.datadir = args.datadir;
        self.client.sequencer_key = args.sequencer_key;
        if let Some(rpc_url) = args.reth_authrpc {
            self.exec.reth.rpc_url = rpc_url;
        }
        if let Some(jwtsecret) = args.reth_jwtsecret {
            self.exec.reth.secret = jwtsecret;
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
