use alloy::{
    primitives::U256,
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use console::Term;

use crate::{rollup::RollupWallet, seed::Seed, signet::SignetWallet};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "balance")]
/// Prints the wallet's current balance(s)
pub struct BalanceArgs {
    /// return the signet balance
    #[argh(switch)]
    signet: bool,
    /// return the rollup balance
    #[argh(switch)]
    rollup: bool,
}

pub async fn balance(args: BalanceArgs, seed: Seed) {
    let term = Term::stdout();
    if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    if args.signet {
        let mut l1w = SignetWallet::new(&seed).unwrap();
        l1w.sync().await.unwrap();
        let balance = l1w.balance();
        let _ = term.write_line(&format!("Total: {}", balance.total()));
        let _ = term.write_line(&format!("  Confirmed: {}", balance.confirmed));
        let _ = term.write_line(&format!("  Trusted pending: {}", balance.trusted_pending));
        let _ = term.write_line(&format!(
            "  Untrusted pending: {}",
            balance.untrusted_pending
        ));
        let _ = term.write_line(&format!("  Immature: {}", balance.immature));
    }

    if args.rollup {
        let l2w = RollupWallet::new(&seed).unwrap();
        let _ = term.write_line("Getting balance...");
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        // 1 BTC = 1 ETH
        let balance_in_btc = balance / U256::from(10u64.pow(18));
        let _ = term.write_line(&format!("\nTotal: {} BTC", balance_in_btc));
    }
}
