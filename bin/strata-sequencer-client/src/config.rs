use std::path::PathBuf;

use serde::Deserialize;

use crate::args::Args;

const DEFAULT_DUTY_POLL_INTERVAL: u64 = 1000;
const DEFAULT_FOLLOWUP_TASK_RETRY: usize = 10;
const DEFAULT_FOLLOWUP_RETRY_DELAY_MS: u64 = 500;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub sequencer_key: PathBuf,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub duty_poll_interval: u64,
    pub followup_tasks_enabled: bool,
    pub followup_task_retry: usize,
    pub followup_retry_delay_ms: u64,
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
            followup_tasks_enabled: args.followup_tasks,
            followup_task_retry: args
                .followup_task_retry
                .unwrap_or(DEFAULT_FOLLOWUP_TASK_RETRY),
            followup_retry_delay_ms: args
                .followup_task_delay_ms
                .unwrap_or(DEFAULT_FOLLOWUP_RETRY_DELAY_MS),
        })
    }

    pub(crate) fn ws_url(&self) -> String {
        format!("ws://{}:{}", self.rpc_host, self.rpc_port)
    }
}
