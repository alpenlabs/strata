use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;

use crate::{
    net_type::{net_type_or_exit, NetworkType},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
    strata::StrataWallet,
};

/// Prints a new address for the internal wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "receive")]
pub struct ReceiveArgs {
    /// either "signet" or "strata"
    #[argh(positional)]
    network_type: String,
}

pub async fn receive(args: ReceiveArgs, seed: Seed, settings: Settings) {
    let network_type = net_type_or_exit(&args.network_type);

    let address = match network_type {
        NetworkType::Signet => {
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .unwrap();
            println!("Syncing signet wallet");
            l1w.sync().await.unwrap();
            println!("Wallet synced");
            let address_info = l1w.reveal_next_address(KeychainKind::External);
            l1w.persist().unwrap();
            address_info.address.to_string()
        }
        NetworkType::Strata => {
            let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();
            l2w.default_signer_address().to_string()
        }
    };

    println!("{address}");
}
