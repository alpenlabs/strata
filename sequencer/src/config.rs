use std::path::PathBuf;

use alpen_vertex_btcio::reader::config::ReaderConfig;
use bitcoin::Network;
use serde::Deserialize;

use crate::args::Args;

#[derive(Deserialize, Debug)]
pub struct RollupConfig {
    pub l1_start_block_height: u64,
    pub l1_follow_distance: u64,
    pub block_time: u64,
    pub rpc_port: u16,
    pub sequencer_key: Option<PathBuf>,
    pub datadir:  PathBuf
}

#[derive(Deserialize, Debug)]
pub struct BitcoinConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
}

#[derive(Deserialize, Debug)]
pub struct FullConfig {
    pub l1_config: BitcoinConfig,
    pub rollup_config: RollupConfig,
    pub reader_config: ReaderConfig,
}

impl FullConfig {
    pub fn new() -> FullConfig {
        Self {
            l1_config: BitcoinConfig {
                rpc_url: String::new(),
                rpc_user: String::new(),
                rpc_password: String::new(),
                network: Network::Regtest,
            },
            rollup_config: RollupConfig { 
                l1_start_block_height: 4, 
                l1_follow_distance: 6,
                rpc_port: 8432,
                block_time: 250, 
                datadir: PathBuf::new(),
                sequencer_key: None 
            },
            reader_config: ReaderConfig {
                max_reorg_depth: 4,
                client_poll_dur_ms: 200,
            }
        }
    }
    pub fn update_from_args(&mut self ,args: &Args) {
        let args = args.clone();
        self.l1_config.rpc_user = args.bitcoind_user;
        self.l1_config.rpc_url = args.bitcoind_host;
        self.rollup_config.rpc_port = args.rpc_port;
        self.l1_config.rpc_password = args.bitcoind_password;
        self.rollup_config.datadir = args.datadir;
        self.rollup_config.sequencer_key = args.sequencer_key;
    }
}

#[cfg(test)]
mod test {
    use crate::config::FullConfig;

    #[test]
    fn config_load_test() {
        let config_string = r#"
            [l1_config]
            rpc_url = "http://localhost:18332"
            rpc_user = "alpen"
            rpc_password = "alpen"
            network = "regtest"

            [rollup_config]
            l1_start_block_height = 4
            l1_follow_distance = 6
            rpc_port = 8432
            block_time = 250
            datadir = "/path/to/data/directory"

            [reader_config]
            max_reorg_depth = 4
            client_poll_dur_ms = 200
        "#;

        assert!(toml::from_str::<FullConfig>(config_string).is_ok());
    }
}

