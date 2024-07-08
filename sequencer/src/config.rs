use bitcoin::Network;

// TODO: Not used now, later associate with a config file
pub struct _RollupConfig {
    pub l1_start_block_height: u64,
    pub l1_rpc_config: _BitcoinConfig,
}

pub struct _BitcoinConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: Network,
}
