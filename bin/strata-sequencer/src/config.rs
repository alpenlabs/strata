use std::path::PathBuf;

use serde::Deserialize;

use crate::args::Args;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub sequencer_key: PathBuf,
    pub rpc_host: String,
    pub rpc_port: u16,
}

impl Config {
    pub(crate) fn from_args(args: &Args) -> Result<Config, String> {
        let args = args.clone();
        Ok(Self {
            sequencer_key: args
                .sequencer_key
                .ok_or_else(|| "args: no --sequencer-key provided".to_string())?,
            rpc_host: args
                .rpc_host
                .ok_or_else(|| "args: no --rpc-host provided".to_string())?,
            rpc_port: args
                .rpc_port
                .ok_or_else(|| "args: no --rpc-port provided".to_string())?,
        })
    }

    pub(crate) fn rpc_url(&self) -> String {
        format!("{}:{}", self.rpc_host, self.rpc_port)
    }
}
