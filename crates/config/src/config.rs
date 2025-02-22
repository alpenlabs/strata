use std::path::PathBuf;

use bitcoin::Network;
use serde::Deserialize;

use crate::{bridge::RelayerConfig, btcio::BtcioConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct FullNodeConfig {
    /// host:port of sequencer rpc
    pub sequencer_rpc: String,
}

// SequencerConfig is empty for now
#[derive(Debug, Clone, Deserialize)]
pub struct SequencerConfig {}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ClientMode {
    FullNode(FullNodeConfig),
    Sequencer(SequencerConfig),
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
    pub client_checkpoint_interval: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BitcoindConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval: Option<u64>,
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
    pub btcio: BtcioConfig,
    pub sync: SyncConfig,
    pub exec: ExecConfig,
    pub relayer: RelayerConfig,
}

#[cfg(test)]
mod test {
    use super::*;

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
            client_poll_dur_ms = 200
            client_checkpoint_interval = 10

            [exec.reth]
            rpc_url = "http://localhost:8551"
            secret = "1234567890abcdef"

            [btcio.reader]
            client_poll_dur_ms = 200

            [btcio.writer]
            write_poll_dur_ms = 200
            fee_policy = "smart"
            reveal_amount = 100
            bundle_interval_ms = 1000

            [btcio.broadcaster]
            poll_interval_ms = 1000

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
        assert!(matches!(
            config.unwrap().client.client_mode,
            ClientMode::Sequencer(..)
        ));

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
            client_poll_dur_ms = 200
            client_checkpoint_interval = 10

            [btcio.reader]
            client_poll_dur_ms = 200

            [btcio.writer]
            write_poll_dur_ms = 200
            fee_policy = "smart"
            reveal_amount = 100
            bundle_interval_ms = 1000

            [btcio.broadcaster]
            poll_interval_ms = 1000

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
