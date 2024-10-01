use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;
use console::Term;

use crate::{
    constants::NETWORK,
    rollup::RollupWallet,
    seed::Seed,
    settings::Settings,
    signet::{EsploraClient, SignetWallet},
};

/// Prints a new address for the internal wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "receive")]
pub struct ReceiveArgs {
    /// prints a new signet address
    #[argh(switch)]
    signet: bool,

    /// prints the rollup address
    #[argh(switch)]
    rollup: bool,
}

pub async fn receive(args: ReceiveArgs, seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    if args.signet && args.rollup {
        let _ = term.write_line("Cannot use both --signet and --rollup options at once");
        std::process::exit(1);
    } else if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    let address = if args.signet {
        let mut l1w = SignetWallet::new(&seed, NETWORK).unwrap();
        let _ = term.write_line("Syncing signet wallet");
        l1w.sync(&esplora).await.unwrap();
        let _ = term.write_line("Wallet synced");
        let address_info = l1w.reveal_next_address(KeychainKind::External);
        l1w.persist().unwrap();
        address_info.address.to_string()
    } else {
        let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).unwrap();
        l2w.default_signer_address().to_string()
    };
    let _ = term.write_line(&address);
}
