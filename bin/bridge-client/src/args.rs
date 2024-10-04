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
        description = "xpriv to be loaded into the bitcoin wallet using the RPC client (default: read from envvar STRATA_OP_XPRIV)"
    )]
    pub xpriv_str: Option<String>,

    #[argh(option, description = "url for the bitcoin RPC client")]
    pub btc_url: String,

    #[argh(option, description = "username for the bitcoin RPC client")]
    pub btc_user: String,

    #[argh(option, description = "password for the bitcoin RPC client")]
    pub btc_pass: String,

    #[argh(option, description = "url for the rollup RPC client")]
    pub rollup_url: String,

    #[argh(
        option,
        description = "bridge duty polling interval in milliseconds (default: rollup block time according to the rollup RPC client)"
    )]
    pub duty_interval: Option<u64>, // default: same as rollup block time

    #[argh(
        option,
        description = "bridge message polling interval in milliseconds (default: half of the block time according to the rollup RPC client)"
    )]
    #[allow(dead_code)] // FIXME: the bridge client also needs to pool for messages
    pub message_interval: Option<u64>, // default: half value of duty

    #[argh(
        option,
        description = "path to the directory where to store the rocksdb databases (default: $HOME/.local/share/strata/)"
    )]
    pub data_dir: Option<String>,

    #[argh(
        option,
        description = "retry count for the rocksdb database (default = 3)"
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
