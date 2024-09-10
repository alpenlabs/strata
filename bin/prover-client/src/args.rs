use argh::FromArgs;

pub(super) const RPC_PORT: usize = 4844;

/// Command-line arguments
#[derive(Debug, FromArgs)]
pub struct Args {
    /// the RPC port to use (optional)
    #[argh(option, default = "RPC_PORT")]
    pub rpc_port: usize,
}

impl Args {
    pub fn get_rpc_url(&self) -> String {
        format!("localhost:{}", self.rpc_port)
    }
}
