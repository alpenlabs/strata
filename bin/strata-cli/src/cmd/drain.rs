use std::str::FromStr;

use alloy::{
    primitives::{Address as StrataAddress, U256},
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use colored::Colorize;
use terrors::OneOf;

use crate::{
    constants::SATS_TO_WEI,
    errors::{InternalError, UserInputError},
    link::{OnchainObject, PrettyPrint},
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, SignetWallet},
    strata::StrataWallet,
};

/// Drains the internal wallet to the provided
/// signet and Strata addresses
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "drain")]
pub struct DrainArgs {
    /// a signet address for signet funds to be drained to
    #[argh(option, short = 's')]
    signet_address: Option<String>,

    /// a Strata address for Strata funds to be drained to
    #[argh(option, short = 'r')]
    strata_address: Option<String>,

    /// override signet fee rate in sat/vbyte. must be >=1
    #[argh(option)]
    fee_rate: Option<u64>,
}

pub async fn drain(
    DrainArgs {
        signet_address,
        strata_address,
        fee_rate,
    }: DrainArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), OneOf<(InternalError, UserInputError)>> {
    if strata_address.is_none() && signet_address.is_none() {
        return Err(OneOf::new(UserInputError::MissingTargetAddress));
    }

    let signet_address = signet_address
        .map(|a| {
            Address::from_str(&a).map_err(|_| OneOf::new(UserInputError::InvalidSignetAddress))
        })
        .transpose()?
        .map(|a| {
            a.require_network(settings.network)
                .map_err(|_| OneOf::new(UserInputError::WrongNetwork))
        })
        .transpose()?;
    let strata_address = strata_address
        .map(|a| {
            StrataAddress::from_str(&a)
                .map_err(|_| OneOf::new(UserInputError::InvalidStrataAddress))
        })
        .transpose()?;

    if let Some(address) = signet_address {
        let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
            .map_err(|e| OneOf::new(InternalError::LoadSignetWallet(format!("{e:?}"))))?;
        l1w.sync()
            .await
            .map_err(|e| OneOf::new(InternalError::SyncSignetWallet(format!("{e:?}"))))?;
        let balance = l1w.balance();
        if balance.untrusted_pending > Amount::ZERO {
            println!(
                "{}",
                "You have pending funds on signet that won't be included in the drain".yellow()
            );
        }
        let fee_rate = get_fee_rate(fee_rate, settings.signet_backend.as_ref()).await;
        log_fee_rate(&fee_rate);

        let mut psbt = {
            let mut builder = l1w.build_tx();
            builder.drain_wallet();
            builder.drain_to(address.script_pubkey());
            builder.fee_rate(fee_rate);
            builder
                .finish()
                .map_err(|e| OneOf::new(InternalError::BuildSignetTxn(format!("{e:?}"))))?
        };
        l1w.sign(&mut psbt, Default::default())
            .map_err(|e| OneOf::new(InternalError::SignSignetTxn(format!("{e:?}"))))?;
        let tx = psbt
            .extract_tx()
            .map_err(|e| OneOf::new(InternalError::ExtractSignetTxn(format!("{e:?}"))))?;
        settings
            .signet_backend
            .broadcast_tx(&tx)
            .await
            .map_err(|e| OneOf::new(InternalError::BroadcastSignetTxn(format!("{e:?}"))))?;
        let txid = tx.compute_txid();
        println!(
            "{}",
            OnchainObject::from(&txid)
                .with_maybe_explorer(settings.mempool_space_endpoint.as_deref())
                .pretty()
        );
        println!("Drained signet wallet to {}", address,);
    }

    if let Some(address) = strata_address {
        let l2w = StrataWallet::new(&seed, &settings.strata_endpoint)
            .map_err(|e| OneOf::new(InternalError::LoadStrataWallet(format!("{e:?}"))))?;
        let balance = l2w
            .get_balance(l2w.default_signer_address())
            .await
            .map_err(|e| OneOf::new(InternalError::FetchStrataBalance(format!("{e:?}"))))?;
        if balance == U256::ZERO {
            println!("No Strata bitcoin to send");
        }

        let estimate_tx = l2w
            .transaction_request()
            .from(l2w.default_signer_address())
            .to(address)
            .value(U256::from(1));

        let gas_price = l2w
            .get_gas_price()
            .await
            .map_err(|e| OneOf::new(InternalError::FetchStrataGasPrice(format!("{e:?}"))))?;
        let gas_estimate = l2w
            .estimate_gas(&estimate_tx)
            .await
            .map_err(|e| OneOf::new(InternalError::EstimateStrataGas(format!("{e:?}"))))?;

        let total_fee = gas_estimate * gas_price;
        let max_send_amount = balance.saturating_sub(U256::from(total_fee));

        let tx = l2w.transaction_request().to(address).value(max_send_amount);

        let res = l2w
            .send_transaction(tx)
            .await
            .map_err(|e| OneOf::new(InternalError::BroadcastStrataTxn(format!("{e:?}"))))?;

        println!(
            "{}",
            OnchainObject::from(res.tx_hash())
                .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
                .pretty()
        );

        println!(
            "Drained {} from Strata wallet to {}",
            Amount::from_sat((max_send_amount / U256::from(SATS_TO_WEI)).wrapping_to()),
            address,
        );
    }

    Ok(())
}
