use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
/// Request some bitcoin from the faucet
pub struct FaucetArgs {
    #[argh(switch)]
    /// request signet bitcoin
    signet: bool,
    #[argh(switch)]
    /// request rollup bitcoin
    rollup: bool,
    #[argh(positional)]
    address: Option<String>
}

pub async fn faucet(args: FaucetArgs) {}
