use argh::FromArgs;

#[derive(FromArgs)]
#[argh(description = "Alpen Vertex sequencer")]
pub struct Args {
    #[argh(option, short = 'B', description = "bitcoind connection url")]
    pub bitcoind: String,

    #[argh(option, short = 'r', description = "JSON-RPC port")]
    pub rpc_port: u16,
}
