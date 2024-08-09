use std::path::PathBuf;

use alpen_express_btcio::reader::{config::ReaderConfig, filter::TxInterest};
use bitcoin::Network;
use serde::Deserialize;

use crate::args::Args;

#[derive(Deserialize, Debug)]
pub struct ClientParams {
    pub rpc_port: u16,
    pub sequencer_key: Option<PathBuf>,
    /// The address to which the inscriptions are spent
    pub sequencer_bitcoin_address: String, // TODO: probably move this to another struct
    pub l2_blocks_fetch_limit: u64,
    pub datadir: PathBuf,
    pub db_retry_count: u16,
}

#[derive(Deserialize, Debug)]
pub struct SyncParams {
    pub l1_follow_distance: u64,
    pub max_reorg_depth: u32,
    pub client_poll_dur_ms: u32,
    pub client_checkpoint_interval: u32,
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
    pub fn from_args(args: &Args) -> Result<Config, String> {
        let args = args.clone();
        Ok(Self {
            bitcoind_rpc: BitcoindParams {
                rpc_url: args
                    .bitcoind_host
                    .ok_or_else(|| "args: no bitcoin --rpc-url provided".to_string())?,
                rpc_user: args
                    .bitcoind_user
                    .ok_or_else(|| "args: no bitcoin --rpc-user provided".to_string())?,
                rpc_password: args
                    .bitcoind_password
                    .ok_or_else(|| "args: no bitcoin --rpc-password provided".to_string())?,
                network: args
                    .network
                    .ok_or_else(|| "args: no bitcoin --network provided".to_string())?,
            },
            client: ClientParams {
                rpc_port: args
                    .rpc_port
                    .ok_or_else(|| "args: no client --rpc-port provided".to_string())?,
                datadir: args
                    .datadir
                    .ok_or_else(|| "args: no client --datadir provided".to_string())?,
                sequencer_key: args.sequencer_key,
                l2_blocks_fetch_limit: 1_000,
                sequencer_bitcoin_address: args
                    .sequencer_bitcoin_address
                    .ok_or_else(|| "args: no --sequencer-bitcion-address provided".to_string())?,
                db_retry_count: 5,
            },
            sync: SyncParams {
                l1_follow_distance: 6,
                max_reorg_depth: 4,
                client_poll_dur_ms: 200,
                client_checkpoint_interval: 10,
            },
            exec: ExecParams {
                reth: RethELParams {
                    rpc_url: args.reth_authrpc.unwrap_or("".to_string()), // TODO: sensible default
                    secret: args.reth_jwtsecret.unwrap_or_default(),      /* TODO: probably
                                                                           * secret should be
                                                                           * Option */
                },
            },
        })
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
        if let Some(db_retry_count) = args.db_retry_count {
            self.client.db_retry_count = db_retry_count;
        }
    }

    pub fn get_reader_config(&self) -> ReaderConfig {
        ReaderConfig {
            max_reorg_depth: self.sync.max_reorg_depth,
            client_poll_dur_ms: self.sync.client_poll_dur_ms,
            tx_interests: vec![TxInterest::TxIdWithPrefix(Vec::new())], /* basically filter all
                                                                         * the txs */
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
            l2_blocks_fetch_limit = 1000
            datadir = "/path/to/data/directory"
            sequencer_bitcoin_address = "some_addr"
            db_retry_count = 5

            [sync]
            l1_follow_distance = 6
            max_reorg_depth = 4
            client_poll_dur_ms = 200
            client_checkpoint_interval = 10

            [exec.reth]
            rpc_url = "http://localhost:8551"
            secret = "1234567890abcdef"
        "#;

        assert!(toml::from_str::<Config>(config_string).is_ok());
    }
}
