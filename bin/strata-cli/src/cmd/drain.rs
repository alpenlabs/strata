use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "drain")]
/// Drains the internal wallet to the provided
/// signet and rollup addresses
pub struct DrainArgs {
    #[argh(positional)]
    signet_address: String,
    #[argh(positional)]
    rollup_address: String
}

pub async fn drain(args: DrainArgs) {}
