use std::{str::FromStr, time::Duration};

use alloy::{
    network::TransactionBuilder, primitives::U256, providers::Provider,
    rpc::types::TransactionInput,
};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use indicatif::ProgressBar;
use strata_primitives::bitcoin_bosd::Descriptor;
use terrors::OneOf;

use crate::{
    constants::{BRIDGE_OUT_AMOUNT, SATS_TO_WEI},
    errors::{
        InvalidSignetAddress, InvalidStrataEndpoint, SignetWalletError, StrataTxError, WrongNetwork,
    },
    handle_or_exit,
    link::{OnchainObject, PrettyPrint},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
    strata::StrataWallet,
};

/// Withdraw 10 BTC from Strata to signet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "withdraw")]
pub struct WithdrawArgs {
    /// the signet address to send funds to. defaults to a new internal wallet address
    #[argh(positional)]
    address: Option<String>,
}

/// Errors that can occur when withdrawing BTC from strata to signet
pub(crate) type WithdrawError = OneOf<(
    InvalidSignetAddress,
    WrongNetwork,
    SignetWalletError,
    InvalidStrataEndpoint,
    StrataTxError,
)>;

pub async fn withdraw(args: WithdrawArgs, seed: Seed, settings: Settings) {
    handle_or_exit!(withdraw_inner(args, seed, settings).await);
}

async fn withdraw_inner(
    args: WithdrawArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), WithdrawError> {
    let address = args
        .address
        .map(|a| {
            Address::from_str(&a)
                .map_err(|_| WithdrawError::new(InvalidSignetAddress(a.clone())))
                .and_then(|addr| {
                    addr.require_network(settings.network).map_err(|_| {
                        OneOf::new(WrongNetwork {
                            address: a.clone(),
                            network: settings.network.to_string(),
                        })
                    })
                })
        })
        .transpose()?;

    let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
        .map_err(|e| {
            WithdrawError::new(SignetWalletError::new("Failed to load signet wallet", e))
        })?;
    l1w.sync().await.map_err(|e| {
        WithdrawError::new(SignetWalletError::new("Failed to sync signet wallet", e))
    })?;
    let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).map_err(WithdrawError::new)?;

    let address = match address {
        Some(a) => a,
        None => {
            let info = l1w.reveal_next_address(KeychainKind::External);
            l1w.persist().map_err(|e| {
                WithdrawError::new(SignetWalletError::new("Failed to persist signet wallet", e))
            })?;
            info.address
        }
    };
    println!("Bridging out {BRIDGE_OUT_AMOUNT} to {address}");

    let bosd: Descriptor = address.into();

    let tx = l2w
        .transaction_request()
        .with_to(settings.bridge_strata_address)
        .with_value(U256::from(BRIDGE_OUT_AMOUNT.to_sat() as u128 * SATS_TO_WEI))
        // calldata for the Strata EVM-BOSD descriptor
        .input(TransactionInput::new(bosd.to_bytes().into()));

    let pb = ProgressBar::new_spinner().with_message("Broadcasting transaction");
    pb.enable_steady_tick(Duration::from_millis(100));
    let res = l2w.send_transaction(tx).await.map_err(|e| {
        WithdrawError::new(StrataTxError::new(
            "Failed to broadcast strata transaction",
            e,
        ))
    })?;
    pb.finish_with_message("Broadcast successful");
    println!(
        "{}",
        OnchainObject::from(res.tx_hash())
            .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
            .pretty(),
    );

    Ok(())
}
