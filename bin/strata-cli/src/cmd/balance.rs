use alloy::{
    primitives::U256,
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::Amount;

use crate::{
    constants::SATS_TO_WEI,
    net_type::{net_type_or_exit, NetworkType},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
    strata::StrataWallet,
};

/// Prints the wallet's current balance(s)
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "balance")]
pub struct BalanceArgs {
    /// either "signet" or "strata"
    #[argh(positional)]
    network_type: String,
}

pub async fn balance(args: BalanceArgs, seed: Seed, settings: Settings) {
    let network_type = net_type_or_exit(&args.network_type);

    if let NetworkType::Signet = network_type {
        let mut l1w =
            SignetWallet::new(&seed, settings.network, settings.signet_backend.clone()).unwrap();
        l1w.sync().await.unwrap();
        let balance = l1w.balance();
        println!("Total: {}", balance.total());
        println!("  Confirmed: {}", balance.confirmed);
        println!("  Trusted pending: {}", balance.trusted_pending);
        println!("  Untrusted pending: {}", balance.untrusted_pending);
        println!("  Immature: {}", balance.immature);
    }

    if let NetworkType::Strata = network_type {
        let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();
        println!("Getting balance...");
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        let balance = Amount::from_sat(
            (balance / U256::from(SATS_TO_WEI))
                .try_into()
                .expect("valid amount"),
        );
        println!("\nTotal: {}", balance);
    }
}
