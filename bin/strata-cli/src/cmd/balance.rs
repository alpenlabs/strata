use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "balance")]
/// Prints the wallet's current balance(s)
pub struct BalanceArgs {
    #[argh(switch)]
    /// return only the signet balance
    signet: bool,
    #[argh(switch)]
    /// return only the rollup balance
    rollup: bool,
}

pub async fn balance(args: BalanceArgs) {}
