use std::{collections::HashMap, path::PathBuf};

use argh::FromArgs;
use strata_primitives::proof::ProofZkVm;

pub(super) const DEV_RPC_PORT: usize = 4844;
pub(super) const DEV_RPC_URL: &str = "0.0.0.0";

/// Command-line arguments used to configure the prover-client in both development and production
/// modes.
#[derive(Debug, FromArgs)]
pub struct Args {
    /// The JSON-RPC port used when running in development mode.
    ///
    /// This port defaults to `DEV_RPC_PORT` and determines the local endpoint port
    /// where the client’s RPC interface is exposed for debugging.
    #[argh(option, description = "JSON-RPC port", default = "DEV_RPC_PORT")]
    pub rpc_port: usize,

    /// The base URL for the JSON-RPC endpoint in development mode.
    ///
    /// Defaults to `DEV_RPC_URL`. When combined with `rpc_port`, it forms the full
    /// RPC endpoint URL for debugging during development.
    #[argh(
        option,
        description = "base JSON-RPC URL",
        default = "DEV_RPC_URL.to_string()"
    )]
    pub rpc_url: String,

    /// The directory path for storing databases and related data.
    ///
    /// This path determines where the client maintains its persistent state.
    #[argh(option, short = 'd', description = "datadir path containing databases")]
    pub datadir: PathBuf,

    /// The URL of the Sequencer RPC endpoint.
    ///
    /// Typically in the format `host:port`.
    #[argh(option, description = "sequencer rpc host:port")]
    pub sequencer_rpc: String,

    /// The URL of the Reth RPC endpoint.
    ///
    /// Typically in the format `host:port`.
    #[argh(option, description = "reth rpc host:port")]
    pub reth_rpc: String,

    /// The host address of the bitcoind RPC endpoint.
    ///
    /// Provide the host (and optionally port) for connecting to a running bitcoind instance.
    #[argh(option, description = "bitcoind RPC host")]
    pub bitcoind_url: String,

    /// The username for the bitcoind RPC authentication.
    #[argh(option, description = "bitcoind RPC user")]
    pub bitcoind_user: String,

    /// The password for the bitcoind RPC authentication.
    #[argh(option, description = "bitcoind RPC password")]
    pub bitcoind_password: String,

    /// The number of Risc0 prover workers to spawn.
    ///
    /// This setting is only available if the `risc0` feature is enabled.
    /// Defaults to `20`.
    #[cfg(feature = "risc0")]
    #[argh(
        option,
        description = "number of risc0 prover workers to spawn",
        default = "20"
    )]
    pub risc0_workers: usize,

    /// The number of SP1 prover workers to spawn.
    ///
    /// This setting is only available if the `sp1` feature is enabled.
    /// Defaults to `20`.
    #[cfg(feature = "sp1")]
    #[argh(
        option,
        description = "number of sp1 prover workers to spawn",
        default = "20"
    )]
    pub sp1_workers: usize,

    /// The number of native prover workers to spawn.
    ///
    /// Defaults to `20`.
    #[argh(
        option,
        description = "number of native prover workers to spawn",
        default = "20"
    )]
    pub native_workers: usize,

    /// The wait time, in seconds, for the prover manager loop.
    ///
    /// Adjust this value to control how frequently the prover manager checks for jobs.
    /// Defaults to `2`.
    #[argh(
        option,
        description = "wait time in seconds for the prover manager loop",
        default = "2"
    )]
    pub loop_interval: u64,

    /// Enables or disables development RPC endpoints.
    ///
    /// Set this to `true` to expose additional RPC endpoints for debugging during development.
    /// Defaults to `true`.
    #[argh(option, description = "enable prover client dev rpc", default = "true")]
    pub enable_dev_rpcs: bool,
}

impl Args {
    /// Constructs the complete development JSON-RPC URL by combining `rpc_url` and `rpc_port`.
    ///
    /// This is used for configuring the client’s RPC interface in development mode.
    pub fn get_dev_rpc_url(&self) -> String {
        format!("{}:{}", self.rpc_url, self.rpc_port)
    }

    /// Returns the Sequencer RPC URL as a `String`.
    ///
    /// Useful for configuring communication with the Sequencer service.
    pub fn get_sequencer_rpc_url(&self) -> String {
        self.sequencer_rpc.to_string()
    }

    /// Returns the Reth RPC URL as a `String`.
    ///
    /// Useful for configuring communication with the Reth service.
    pub fn get_reth_rpc_url(&self) -> String {
        self.reth_rpc.to_string()
    }

    /// Formats and returns the bitcoind RPC URL prefixed with `http://`.
    ///
    /// Useful for establishing a connection to the bitcoind RPC endpoint.
    pub fn get_btc_rpc_url(&self) -> String {
        format!("http://{}", self.bitcoind_url)
    }

    /// Returns a map of proof VMs to the number of workers assigned to each, depending on enabled
    /// features.
    ///
    /// This function populates the `HashMap` based on which features are enabled at compile time.
    /// For example, if the `sp1` or `risc0` features are enabled, corresponding entries will be
    /// included with their configured number of worker threads.
    pub fn get_workers(&self) -> HashMap<ProofZkVm, usize> {
        let mut workers = HashMap::new();
        workers.insert(ProofZkVm::Native, self.native_workers);

        #[cfg(feature = "sp1")]
        {
            workers.insert(ProofZkVm::SP1, self.sp1_workers);
        }

        #[cfg(feature = "risc0")]
        {
            workers.insert(ProofZkVm::Risc0, self.risc0_workers);
        }

        workers
    }
}
