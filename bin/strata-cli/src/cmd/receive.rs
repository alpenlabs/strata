use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "receive")]
/// Prints a new address for the internal wallet
pub struct ReceiveArgs {
    #[argh(switch)]
    /// prints a new signet address
    signet: bool,
    #[argh(switch)]
    /// prints the rollup address
    rollup: bool
}

pub async fn receive(args: ReceiveArgs) {}
