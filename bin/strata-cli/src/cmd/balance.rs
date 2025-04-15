use alloy::{
    primitives::U256,
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::Amount;
use terrors::OneOf;

use crate::{
    constants::SATS_TO_WEI,
    errors::{InternalError, UserInputError},
    net_type::NetworkType,
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

pub async fn balance(
    args: BalanceArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), OneOf<(InternalError, UserInputError)>> {
    let network_type = args.network_type.parse().map_err(OneOf::new)?;

    if let NetworkType::Signet = network_type {
        let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
            .map_err(|e| OneOf::new(InternalError::LoadSignetWallet(format!("{e:?}"))))?;

        l1w.sync()
            .await
            .map_err(|e| OneOf::new(InternalError::SyncSignetWallet(format!("{e:?}"))))?;

        let balance = l1w.balance();
        println!("Total: {}", balance.total());
        println!("  Confirmed: {}", balance.confirmed);
        println!("  Trusted pending: {}", balance.trusted_pending);
        println!("  Untrusted pending: {}", balance.untrusted_pending);
        println!("  Immature: {}", balance.immature);
    }

    if let NetworkType::Strata = network_type {
        let l2w = StrataWallet::new(&seed, &settings.strata_endpoint)
            .map_err(|e| OneOf::new(InternalError::LoadStrataWallet(format!("{e:?}"))))?;
        println!("Getting balance...");
        let raw_balance = l2w
            .get_balance(l2w.default_signer_address())
            .await
            .map_err(|e| OneOf::new(InternalError::FetchStrataBalance(format!("{e:?}"))))?;
        let sats = (raw_balance / U256::from(SATS_TO_WEI))
            .try_into()
            .expect("to fit into u64");
        let balance = Amount::from_sat(sats);

        println!("\nTotal: {}", balance);
    }
    Ok(())
}
