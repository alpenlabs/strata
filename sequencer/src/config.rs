use std::path::PathBuf;

use alpen_express_btcio::reader::config::ReaderConfig;
use alpen_express_primitives::relay::types::RelayerConfig;
use bitcoin::Network;
use serde::Deserialize;
use tracing::warn;

use crate::args::Args;

#[derive(Debug, Deserialize)]
pub struct SequencerConfig {
    /// path to sequencer root key
    pub sequencer_key: PathBuf,
    /// address with funds for sequencer transactions
    pub sequencer_bitcoin_address: String,
}

#[derive(Debug, Deserialize)]
pub struct FullNodeConfig {
    /// host:port of sequencer rpc
    pub sequencer_rpc: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ClientMode {
    Sequencer(SequencerConfig),
    FullNode(FullNodeConfig),
}

#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    pub rpc_host: String,
    pub rpc_port: u16,
    #[serde(flatten)]
    pub client_mode: ClientMode,
    pub l2_blocks_fetch_limit: u64,
    pub datadir: PathBuf,
    pub db_retry_count: u16,
    #[serde(with = "hex::serde")]
    pub seq_pubkey: [u8; 32],
}

#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    pub l1_follow_distance: u64,
    pub max_reorg_depth: u32,
    pub client_poll_dur_ms: u32,
    pub client_checkpoint_interval: u32,
}

#[derive(Debug, Deserialize)]
pub struct BitcoindConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
}

#[derive(Debug, Deserialize)]
pub struct RethELConfig {
    pub rpc_url: String,
    pub secret: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct ExecConfig {
    pub reth: RethELConfig,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub client: ClientConfig,
    pub bitcoind_rpc: BitcoindConfig,
    pub sync: SyncConfig,
    pub exec: ExecConfig,
    pub relayer: RelayerConfig,
}

impl Config {
    pub fn from_args(args: &Args) -> Result<Config, String> {
        let args = args.clone();
        Ok(Self {
            bitcoind_rpc: BitcoindConfig {
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
            client: ClientConfig {
                rpc_host: args
                    .rpc_host
                    .ok_or_else(|| "args: no client --rpc-host provided".to_string())?,
                rpc_port: args
                    .rpc_port
                    .ok_or_else(|| "args: no client --rpc-port provided".to_string())?,
                datadir: args
                    .datadir
                    .ok_or_else(|| "args: no client --datadir provided".to_string())?,
                client_mode: {
                    if let (Some(sequencer_key), Some(sequencer_bitcoin_address)) =
                        (args.sequencer_key, args.sequencer_bitcoin_address)
                    {
                        ClientMode::Sequencer(SequencerConfig {
                            sequencer_key,
                            sequencer_bitcoin_address,
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
                seq_pubkey: {
                    if let Some(seq_identity) = args.seq_pubkey {
                        Self::parse_seq_identity_from_arg(&seq_identity)?
                    } else {
                        return Err("arg: --seq-pubkey not provided".to_string());
                    }
                },
            },
            sync: SyncConfig {
                l1_follow_distance: 6,
                max_reorg_depth: 4,
                client_poll_dur_ms: 200,
                client_checkpoint_interval: 10,
            },
            exec: ExecConfig {
                reth: RethELConfig {
                    rpc_url: args.reth_authrpc.unwrap_or("".to_string()), // TODO: sensible default
                    secret: args.reth_jwtsecret.unwrap_or_default(),      /* TODO: probably
                                                                           * secret should be
                                                                           * Option */
                },
            },
            relayer: RelayerConfig {
                refresh_interval: 10,
                stale_duration: 120,
                relay_misc: true,
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
        if let Some(rpc_host) = args.rpc_host {
            self.client.rpc_host = rpc_host;
        }
        if let Some(rpc_port) = args.rpc_port {
            self.client.rpc_port = rpc_port;
        }
        if let Some(datadir) = args.datadir {
            self.client.datadir = datadir;
        }
        // sequencer_key has priority over sequencer_rpc if both are provided

        if let (Some(sequencer_key), Some(sequencer_bitcoin_address)) =
            (args.sequencer_key, args.sequencer_bitcoin_address)
        {
            self.client.client_mode = ClientMode::Sequencer(SequencerConfig {
                sequencer_key,
                sequencer_bitcoin_address,
            });
        } else if let Some(sequencer_rpc) = args.sequencer_rpc {
            self.client.client_mode = ClientMode::FullNode(FullNodeConfig { sequencer_rpc });
        }
        if let Some(rpc_url) = args.reth_authrpc {
            self.exec.reth.rpc_url = rpc_url;
        }
        if let Some(jwtsecret) = args.reth_jwtsecret {
            self.exec.reth.secret = jwtsecret;
        }
        if let Some(db_retry_count) = args.db_retry_count {
            self.client.db_retry_count = db_retry_count;
        }

        if let Some(seq_identity) = args.seq_pubkey {
            let seq_identity_in_config = self.client.seq_pubkey;
            self.client.seq_pubkey = Self::parse_seq_identity_from_arg(&seq_identity)
                .unwrap_or_else(|_| {
                    warn!("ignoring invalid --seq-pubkey from args");

                    seq_identity_in_config
                });
        }
    }

    pub fn get_reader_config(&self) -> ReaderConfig {
        ReaderConfig {
            max_reorg_depth: self.sync.max_reorg_depth,
            client_poll_dur_ms: self.sync.client_poll_dur_ms,
        }
    }

    fn parse_seq_identity_from_arg(seq_identity: &str) -> Result<[u8; 32], String> {
        let seq_identity = hex::decode(seq_identity);
        if seq_identity.is_err() {
            return Err("args: invalid --seq-pubkey provided".to_string());
        }

        let seq_identity = seq_identity.expect("must be present");
        let seq_identity = TryInto::<[u8; 32]>::try_into(seq_identity);

        if seq_identity.is_err() {
            return Err("args: invalid --seq-pubkey length provided".to_string());
        }

        Ok(seq_identity.expect("must be valid"))
    }
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
            "should be able to load fullnode TOML config but got: {:?}",
            config.err()
        );
    }
}
