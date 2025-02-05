use std::path::PathBuf;

use argh::FromArgs;

#[derive(Debug, Clone, FromArgs)]
#[argh(description = "Alpen Strata sequencer")]
pub(crate) struct Args {
    #[argh(option, short = 'k', description = "path to sequencer root key")]
    pub sequencer_key: Option<PathBuf>,

    #[argh(option, short = 'h', description = "JSON-RPC host")]
    pub rpc_host: Option<String>,

    #[argh(option, short = 'p', description = "JSON-RPC port")]
    pub rpc_port: Option<u16>,

    #[argh(option, short = 'i', description = "poll interval for duties in ms")]
    pub duty_poll_interval: Option<u64>,

    #[argh(switch, short = 'f', description = "enable follow-up tasks")]
    pub followup_tasks: bool,

    #[argh(option, short = 'r', description = "follow-up task retry count")]
    pub followup_task_retry: Option<usize>,

    #[argh(option, short = 'd', description = "follow-up task retry delay in ms")]
    pub followup_task_delay_ms: Option<u64>,
}
