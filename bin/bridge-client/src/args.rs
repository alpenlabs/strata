//! Parses command-line arguments for the bridge-client CLI.

use std::fmt::Display;

use argh::FromArgs;

use crate::errors::InitError;

#[derive(Debug, FromArgs)]
#[argh(name = "strata-bridge-client")]
#[argh(description = "The bridge client for Strata")]
pub(crate) struct Cli {
    /// Specifies the mode to run the client in: either Operator (alias: op) or Challenger (alias:
    /// ch).
    #[argh(
        positional,
        description = "what mode to run the client in, either Operator (alias: op) or Challenger (alias: ch)"
    )]
    pub mode: String,

    /// Path to the directory where RocksDB databases are stored.
    /// Defaults to `$HOME/.local/share/strata/` if not specified.
    #[argh(
        option,
        description = "path to the directory where to store the rocksdb databases (default: $HOME/.local/share/strata/)"
    )]
    pub datadir: Option<String>,

    /// Master operator key in xpriv format. Defaults to the environment variable
    /// `STRATA_OP_MASTER_XPRIV` if not provided.
    #[argh(
        option,
        description = "xpriv to be used as the master operator's key (default: envvar STRATA_OP_MASTER_XPRIV)"
    )]
    pub master_xpriv: Option<String>,

    /// Path to the file containing the master operator's xpriv.
    /// Should not be used with the `--master-xpriv` option or `STRATA_OP_MASTER_XPRIV` environment
    /// variable.
    #[argh(
        option,
        description = "path to the file containing the master operator's xpriv (don't use with --master-xpriv or the envvar STRATA_OP_MASTER_XPRIV)"
    )]
    pub master_xpriv_path: Option<String>,

    /// Host address for the RPC server. Defaults to `127.0.0.1` if not specified.
    #[argh(
        option,
        description = "host to run the rpc server on (default: 127.0.0.1)"
    )]
    pub rpc_host: Option<String>,

    /// Port number for the RPC server. Defaults to `4781` if not specified.
    #[argh(option, description = "port to run the rpc server on (default: 4781)")]
    pub rpc_port: Option<u32>,

    /// URL for the Bitcoin RPC endpoint.
    #[argh(option, description = "url for the bitcoin RPC")]
    pub btc_url: String,

    /// Username for accessing the Bitcoin RPC.
    #[argh(option, description = "username for bitcoin RPC")]
    pub btc_user: String,

    /// Password for accessing the Bitcoin RPC.
    #[argh(option, description = "password for bitcoin RPC")]
    pub btc_pass: String,

    /// URL for the Rollup RPC server.
    #[argh(option, description = "url for the rollup RPC server")]
    pub rollup_url: String,

    /// Interval for polling bridge duties in milliseconds.
    /// Defaults to the block time according to the Rollup RPC.
    #[argh(
        option,
        description = "bridge duty polling interval in milliseconds (default: block time according to rollup RPC)"
    )]
    pub duty_interval: Option<u64>,

    /// Interval for polling bridge messages in milliseconds.
    /// Defaults to half the block time according to the Rollup RPC.
    #[argh(
        option,
        description = "bridge message polling interval in milliseconds (default: half of the block time according to the rollup RPC client)"
    )]
    pub message_interval: Option<u64>,

    /// Number of retries for RocksDB database operations. Defaults to `3`.
    #[argh(
        option,
        description = "retry count for the rocksdb database (default: 3)"
    )]
    pub retry_count: Option<u16>,

    /// Timeout duration for duties in seconds. Defaults to `600`.
    #[argh(
        option,
        description = "duty timeout duration in seconds (default: 600)"
    )]
    pub duty_timeout_duration: Option<u64>,

    /// Max retries for when RPC server fails during duty polling
    #[argh(
        option,
        description = "max retries for when RPC server fails during duty polling"
    )]
    pub max_rpc_retry_count: Option<u16>,
}

/// Represents the operational mode of the client.
///
/// - `Operator`: Handles deposits, withdrawals, and challenges.
/// - `Challenger`: Verifies and challenges Operator claims.
#[derive(Debug, Clone)]
pub(super) enum OperationMode {
    /// Run client in Operator mode to handle deposits, withdrawals, and challenging.
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
