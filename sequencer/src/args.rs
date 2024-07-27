use std::path::PathBuf;

use argh::FromArgs;

#[derive(Clone, FromArgs)]
#[argh(description = "Alpen Vertex sequencer")]
pub struct Args {
    // TODO: add a rollup_config file arg
    #[argh(
        option,
        short = 'd',
        description = "datadir path that will contain databases"
    )]
    pub datadir: PathBuf,

    #[argh(option, short = 'r', description = "JSON-RPC port")]
    pub rpc_port: u16,

    #[argh(option, description = "bitcoind RPC host")]
    pub bitcoind_host: String,

    #[argh(option, description = "bitcoind RPC user")]
    pub bitcoind_user: String,

    #[argh(option, description = "bitcoind RPC password")]
    pub bitcoind_password: String,

    #[argh(option, short = 'c', description = "path to configuration")]
    pub config: Option<PathBuf>,

    #[argh(
        option,
        short = 'n',
        description = "L1 network to run on",
        default = "\"regtest\".to_owned()"
    )]
    pub network: String,

    #[argh(option, short = 'k', description = "path to sequencer root key")]
    pub sequencer_key: Option<PathBuf>,

    #[argh(option, description = "reth authrpc host:port")]
    pub reth_authrpc: Option<String>,

    #[argh(option, description = "path to reth authrpc jwtsecret")]
    pub reth_jwtsecret: Option<PathBuf>,
}
