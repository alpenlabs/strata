use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "bridge-in")]
/// Bridge 10 BTC from signet to the rollup
pub struct BridgeInArgs {
    #[argh(positional)]
    rollup_address: Option<String>
}

pub async fn bridge_in(args: BridgeInArgs) {}
