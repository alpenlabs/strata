//! Parses command-line arguments for the bridge-client CLI.
use std::fmt::Display;

use argh::FromArgs;

use crate::errors::InitError;

#[derive(Debug, FromArgs)]
#[argh(name = "strata-bridge-client")]
#[argh(description = "The bridge client for Strata")]
pub(crate) struct Cli {
    #[argh(
        positional,
        description = "what mode to run the client in, either Operator (alias: op) or Challenger (alias: ch)"
    )]
    pub mode: String,

    #[argh(
        option,
        description = "path to the directory where to store the rocksdb databases (default: $HOME/.local/share/strata/)"
    )]
    pub datadir: Option<String>,

    #[argh(
        option,
        description = "xpriv to be used as the master operator's key (default: envvar STRATA_OP_MASTER_XPRIV)"
    )]
    pub master_xpriv: Option<String>,

    #[argh(
        option,
        description = "path to the file containing the master operator's xpriv (don't use with --master-xpriv or the envvar STRATA_OP_MASTER_XPRIV)"
    )]
    pub master_xpriv_path: Option<String>,

    #[argh(
        option,
        description = "host to run the rpc server on (default: 127.0.0.1)"
    )]
    pub rpc_host: Option<String>,

    #[argh(option, description = "port to run the rpc server on (default: 4781)")]
    pub rpc_port: Option<u32>,

    #[argh(option, description = "url for the bitcoin RPC")]
    pub btc_url: String,

    #[argh(option, description = "username for bitcoin RPC")]
    pub btc_user: String,

    #[argh(option, description = "password for bitcoin RPC")]
    pub btc_pass: String,

    #[argh(option, description = "url for the rollup RPC server")]
    pub rollup_url: String,

    #[argh(
        option,
        description = "bridge duty polling interval in milliseconds (default: block time according to rollup RPC)"
    )]
    pub duty_interval: Option<u64>, // default: same as rollup block time

    #[argh(
        option,
        description = "bridge message polling interval in milliseconds (default: half of the block time according to the rollup RPC client)"
    )]
    pub message_interval: Option<u64>, // default: half of the rollup block time

    #[argh(
        option,
        description = "retry count for the rocksdb database (default: 3)"
    )]
    pub retry_count: Option<u16>,
}

#[derive(Debug, Clone)]
pub(super) enum OperationMode {
    /// Run client in Operator mode to handle deposits, withdrawals and challenging.
    Operator,

    /// Run client in Challenger mode to verify/challenge Operator claims.
    Challenger,
}

impl Display for OperationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationMode::Operator => write!(f, "operator"),
            OperationMode::Challenger => write!(f, "challenger"),
        }
    }
}

impl TryInto<OperationMode> for String {
    type Error = InitError;

    fn try_into(self) -> Result<OperationMode, Self::Error> {
        match self.as_ref() {
            "operator" | "op" => Ok(OperationMode::Operator),
            "challenger" | "ch" => Ok(OperationMode::Challenger),
            other => Err(InitError::InvalidMode(other.to_string())),
        }
    }
}
