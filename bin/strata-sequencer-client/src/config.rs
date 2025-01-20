use std::path::PathBuf;

use serde::Deserialize;

use crate::args::Args;

const DEFAULT_DUTY_POLL_INTERVAL: u64 = 1000;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub sequencer_key: PathBuf,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub duty_poll_interval: u64,
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
            duty_poll_interval: args
                .duty_poll_interval
                .unwrap_or(DEFAULT_DUTY_POLL_INTERVAL),
        })
    }

    pub(crate) fn ws_url(&self) -> String {
        format!("ws://{}:{}", self.rpc_host, self.rpc_port)
    }
}
