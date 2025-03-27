use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;

use crate::{
    alpen::AlpenWallet,
    net_type::{net_type_or_exit, NetworkType},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
};

/// Prints a new address for the internal wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "receive")]
pub struct ReceiveArgs {
    /// either "signet" or "alpen"
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
        NetworkType::Alpen => {
            let l2w = AlpenWallet::new(&seed, &settings.alpen_endpoint).unwrap();
            l2w.default_signer_address().to_string()
        }
    };

    println!("{address}");
}
