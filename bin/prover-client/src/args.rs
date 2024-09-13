use argh::FromArgs;

pub(super) const RPC_PORT: usize = 4844;

/// Command-line arguments
#[derive(Debug, FromArgs)]

pub struct Args {
    /// the RPC port to use (optional)
    #[argh(option, description = "JSON-RPC port", default = "RPC_PORT")]
    pub rpc_port: usize,

    #[argh(option, description = "sequencer rpc host:port")]
    pub sequencer_rpc: String,

    #[argh(option, description = "reth rpc host:port")]
    pub reth_rpc: String,
}

impl Args {
    pub fn get_rpc_url(&self) -> String {
        format!("localhost:{}", self.rpc_port)
    }

    pub fn get_sequencer_rpc_url(&self) -> String {
        self.sequencer_rpc.to_string()
    }

    pub fn get_reth_rpc_url(&self) -> String {
        self.reth_rpc.to_string()
    }
}
