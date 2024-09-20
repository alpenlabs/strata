use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
/// Send some bitcoin from the internal wallet.
pub struct SendArgs {
    #[argh(switch)]
    /// send via signet
    signet: bool,
    #[argh(switch)]
    /// send via rollup
    rollup: bool,
    #[argh(positional)]
    amount: u64,
    #[argh(positional)]
    address: String
}

pub async fn send(args: SendArgs) {}
