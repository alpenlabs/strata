use std::{fs, path::PathBuf};

use argh::FromArgs;
use strata_primitives::params::RollupParams;

pub(super) const DEV_RPC_PORT: usize = 4844;
pub(super) const DEV_RPC_URL: &str = "0.0.0.0";

/// Command-line arguments
#[derive(Debug, FromArgs)]

pub struct Args {
    /// The JSON-RPC port used in development mode.
    ///
    /// This port is optional and defaults to `DEV_RPC_PORT`.
    /// It allows the client to expose the RPC endpoint for debugging purposes when running in dev
    /// mode.
    #[argh(option, description = "JSON-RPC port", default = "DEV_RPC_PORT")]
    pub rpc_port: usize,

    /// The JSON-RPC URL used in development mode.
    ///
    /// This URL is the endpoint where the client exposes the RPC interface for debugging when
    /// running in dev mode. It defaults to `DEV_RPC_URL`.
    #[argh(
        option,
        description = "JSON-RPC URL",
        default = "DEV_RPC_URL.to_string()"
    )]
    pub rpc_url: String,

    #[argh(option, description = "sequencer rpc host:port")]
    pub sequencer_rpc: String,

    #[argh(option, description = "reth rpc host:port")]
    pub reth_rpc: String,

    #[argh(option, description = "bitcoind RPC host")]
    pub bitcoind_url: String,

    #[argh(option, description = "bitcoind RPC user")]
    pub bitcoind_user: String,

    #[argh(option, description = "bitcoind RPC password")]
    pub bitcoind_password: String,

    #[argh(option, short = 'p', description = "custom rollup config path")]
    pub rollup_params: PathBuf,

    #[argh(option, description = "enable prover client dev rpc", default = "true")]
    pub enable_dev_rpcs: bool,

    #[argh(
        option,
        description = "enable prover client checkpoint runner",
        default = "false"
    )]
    pub enable_checkpoint_runner: bool,
}

impl Args {
    /// Constructs the full JSON-RPC URL for development mode by combining the base URL and port.
    ///
    /// This method formats the development RPC URL by appending the `rpc_port` to the `rpc_url`.
    /// It is primarily used to configure the client to expose the RPC endpoint for debugging
    /// purposes when running in development mode.
    pub fn get_dev_rpc_url(&self) -> String {
        format!("{}:{}", self.rpc_url, self.rpc_port)
    }

    pub fn get_sequencer_rpc_url(&self) -> String {
        self.sequencer_rpc.to_string()
    }

    pub fn get_reth_rpc_url(&self) -> String {
        self.reth_rpc.to_string()
    }

    pub fn get_btc_rpc_url(&self) -> String {
        format!("http://{}", self.bitcoind_url)
    }

    /// Resolves the rollup params file to use, from a path, and validates
    /// it to ensure it passes sanity checks.
    pub fn resolve_and_validate_rollup_params(&self) -> anyhow::Result<RollupParams> {
        let json = fs::read_to_string(&self.rollup_params)?;
        let rollup_params = serde_json::from_str::<RollupParams>(&json)?;
        rollup_params.check_well_formed()?;
        Ok(rollup_params)
    }
}
