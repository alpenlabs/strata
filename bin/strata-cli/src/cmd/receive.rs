use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;
use console::Term;

use crate::{rollup::RollupWallet, seed::Seed, signet::SignetWallet};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "receive")]
/// Prints a new address for the internal wallet
pub struct ReceiveArgs {
    #[argh(switch)]
    /// prints a new signet address
    signet: bool,
    #[argh(switch)]
    /// prints the rollup address
    rollup: bool,
}

pub async fn receive(args: ReceiveArgs, seed: Seed) {
    let term = Term::stdout();
    if args.signet && args.rollup {
        let _ = term.write_line("Cannot use both --signet and --rollup options at once");
        std::process::exit(1);
    } else if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    let address = if args.signet {
        let mut l1w = SignetWallet::new(&seed).unwrap();
        let _ = term.write_line("Syncing signet wallet");
        l1w.sync().await.unwrap();
        let _ = term.write_line("Wallet synced");
        let address_info = l1w.reveal_next_address(KeychainKind::External);
        l1w.persist().unwrap();
        address_info.address.to_string()
    } else if args.rollup {
        let l2w = RollupWallet::new(&seed).unwrap();
        l2w.default_signer_address().to_string()
    } else {
        unreachable!()
    };
    let _ = term.write_line(&address);
}
