use std::path::PathBuf;

use argh::FromArgs;
use bitcoin::Network;

#[derive(Debug, Clone, FromArgs)]
#[argh(description = "Alpen Vertex sequencer")]
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

    #[argh(option, description = "reth authrpc host:port")]
    pub reth_authrpc: Option<String>,

    #[argh(option, description = "path to reth authrpc jwtsecret")]
    pub reth_jwtsecret: Option<PathBuf>,
}
