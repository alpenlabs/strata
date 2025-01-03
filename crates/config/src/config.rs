use std::path::PathBuf;

use bitcoin::Network;
use serde::Deserialize;

use crate::RelayerConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct SequencerConfig {
    /// path to sequencer root key
    pub sequencer_key: PathBuf,
    /// address with funds for sequencer transactions
    pub sequencer_bitcoin_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FullNodeConfig {
    /// host:port of sequencer rpc
    pub sequencer_rpc: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ClientMode {
    Sequencer(SequencerConfig),
    FullNode(FullNodeConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
    pub rpc_host: String,
    pub rpc_port: u16,
    #[serde(flatten)]
    pub client_mode: ClientMode,
    pub l2_blocks_fetch_limit: u64,
    pub datadir: PathBuf,
    pub db_retry_count: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncConfig {
    pub l1_follow_distance: u64,
    pub max_reorg_depth: u32,
    pub client_poll_dur_ms: u32,
    pub client_checkpoint_interval: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitcoindConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RethELConfig {
    pub rpc_url: String,
    pub secret: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecConfig {
    pub reth: RethELConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub client: ClientConfig,
    pub bitcoind_rpc: BitcoindConfig,
    pub sync: SyncConfig,
    pub exec: ExecConfig,
    pub relayer: RelayerConfig,
}

#[cfg(test)]
mod test {
    use crate::config::Config;

    #[test]
    fn test_config_load() {
        let config_string_sequencer = r#"
            [bitcoind_rpc]
            rpc_url = "http://localhost:18332"
            rpc_user = "alpen"
            rpc_password = "alpen"
            network = "regtest"

            [client]
            rpc_host = "0.0.0.0"
            rpc_port = 8432
            l2_blocks_fetch_limit = 1000
            datadir = "/path/to/data/directory"
            sequencer_bitcoin_address = "some_addr"
            sequencer_key = "/path/to/sequencer_key"
            seq_pubkey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            db_retry_count = 5

            [sync]
            l1_follow_distance = 6
            max_reorg_depth = 4
            client_poll_dur_ms = 200
            client_checkpoint_interval = 10

            [exec.reth]
            rpc_url = "http://localhost:8551"
            secret = "1234567890abcdef"

            [relayer]
            refresh_interval = 10
            stale_duration = 120
            relay_misc = true
        "#;

        let config = toml::from_str::<Config>(config_string_sequencer);
        assert!(
            config.is_ok(),
            "should be able to load sequencer TOML config but got: {:?}",
            config.err()
        );

        let config_string_fullnode = r#"
            [bitcoind_rpc]
            rpc_url = "http://localhost:18332"
            rpc_user = "alpen"
            rpc_password = "alpen"
            network = "regtest"

            [client]
            rpc_host = "0.0.0.0"
            rpc_port = 8432
            l2_blocks_fetch_limit = 1000
            datadir = "/path/to/data/directory"
            sequencer_bitcoin_address = "some_addr"
            sequencer_rpc = "9.9.9.9:8432"
            seq_pubkey = "123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0"
            db_retry_count = 5

            [sync]
            l1_follow_distance = 6
            max_reorg_depth = 4
            client_poll_dur_ms = 200
            client_checkpoint_interval = 10

            [exec.reth]
            rpc_url = "http://localhost:8551"
            secret = "1234567890abcdef"

            [relayer]
            refresh_interval = 10
            stale_duration = 120
            relay_misc = true
        "#;

        let config = toml::from_str::<Config>(config_string_fullnode);
        assert!(
            config.is_ok(),
            "should be able to load full-node TOML config but got: {:?}",
            config.err()
        );
    }
}
