use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "bridge-out")]
/// Bridge 10 BTC from the rollup to signet
pub struct BridgeOutArgs {
    #[argh(positional)]
    p2tr_address: Option<String>
}

pub async fn bridge_out(args: BridgeOutArgs) {}
