use bitcoin::Network;

pub struct RollupConfig {
    pub l1_start_block_height: u64,
    pub l1_rpc_config: BitcoinConfig,
}

pub struct BitcoinConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub zmq_endpoint: String,
    pub network: Network,
    // TODO: network
}

// FIXME: These are just for local dev, remove later
impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:18444".to_string(),
            rpc_user: "rpcuser".to_string(),
            rpc_password: "rpcpassword".to_string(),
            zmq_endpoint: "tcp://127.0.0.1:29000".to_string(),
            network: Network::Regtest,
        }
    }
}

impl Default for RollupConfig {
    fn default() -> Self {
        Self {
            l1_start_block_height: 1,
            l1_rpc_config: BitcoinConfig::default(),
        }
    }
}
