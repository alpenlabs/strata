use alloy::providers::WalletProvider;
use argh::FromArgs;
use bdk_wallet::KeychainKind;
use terrors::OneOf;

use crate::{
    errors::{InternalError, UserInputError},
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

pub async fn receive(
    args: ReceiveArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), OneOf<(InternalError, UserInputError)>> {
    let network_type = args.network_type.parse().map_err(OneOf::new)?;

    let address = match network_type {
        NetworkType::Signet => {
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .map_err(|e| OneOf::new(InternalError::LoadSignetWallet(format!("{e:?}"))))?;

            println!("Syncing signet wallet...");
            l1w.sync()
                .await
                .map_err(|e| OneOf::new(InternalError::SyncSignetWallet(format!("{e:?}"))))?;
            println!("Wallet synced.");

            let address_info = l1w.reveal_next_address(KeychainKind::External);

            l1w.persist()
                .map_err(|e| OneOf::new(InternalError::PersistSignetWallet(format!("{e:?}"))))?;

            address_info.address.to_string()
        }
        NetworkType::Strata => {
            let l2w = StrataWallet::new(&seed, &settings.strata_endpoint)
                .map_err(|e| OneOf::new(InternalError::LoadStrataWallet(format!("{e:?}"))))?;
            l2w.default_signer_address().to_string()
        }
    };

    println!("{address}");
    Ok(())
}
