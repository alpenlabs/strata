use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;
use terrors::OneOf;

use crate::{
    errors::{InvalidStrataEndpoint, SignetWalletError, UnsupportedNetwork},
    handle_or_exit,
    net_type::NetworkType,
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

/// Errors that can occur when displaying receive address
pub(crate) type ReceiveError =
    OneOf<(InvalidStrataEndpoint, UnsupportedNetwork, SignetWalletError)>;

pub async fn receive(args: ReceiveArgs, seed: Seed, settings: Settings) {
    handle_or_exit!(receive_inner(args, seed, settings).await);
}

async fn receive_inner(
    args: ReceiveArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), ReceiveError> {
    let network_type = args.network_type.parse().map_err(OneOf::new)?;

    let address = match network_type {
        NetworkType::Signet => {
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .map_err(|e| {
                        ReceiveError::new(SignetWalletError::new("Failed to load signet wallet", e))
                    })?;

            println!("Syncing signet wallet...");
            l1w.sync().await.map_err(|e| {
                ReceiveError::new(SignetWalletError::new("Failed to sync signet wallet", e))
            })?;
            println!("Wallet synced.");

            let address_info = l1w.reveal_next_address(KeychainKind::External);

            l1w.persist().map_err(|e| {
                ReceiveError::new(SignetWalletError::new("Failed to persist signet wallet", e))
            })?;

            address_info.address.to_string()
        }
        NetworkType::Strata => {
            let l2w =
                StrataWallet::new(&seed, &settings.strata_endpoint).map_err(ReceiveError::new)?;
            l2w.default_signer_address().to_string()
        }
    };

    println!("{address}");
    Ok(())
}
