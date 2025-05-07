use std::str::FromStr;

use alloy::{
    primitives::{Address as AlpenAddress, U256},
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use colored::Colorize;

use crate::{
    alpen::AlpenWallet,
    constants::SATS_TO_WEI,
    link::{OnchainObject, PrettyPrint},
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, SignetWallet},
};

/// Drains the internal wallet to the provided
/// signet and Alpen addresses
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "drain")]
pub struct DrainArgs {
    /// a signet address for signet funds to be drained to
    #[argh(option, short = 's')]
    signet_address: Option<String>,

    /// a Alpen address for Alpen funds to be drained to
    #[argh(option, short = 'r')]
    alpen_address: Option<String>,

    /// override signet fee rate in sat/vbyte. must be >=1
    #[argh(option)]
    fee_rate: Option<u64>,
}

pub async fn drain(
    DrainArgs {
        signet_address,
        alpen_address,
        fee_rate,
    }: DrainArgs,
    seed: Seed,
    settings: Settings,
) {
    if alpen_address.is_none() && signet_address.is_none() {
        println!("Either signet or Alpen address should be provided");
    }

    let signet_address = signet_address.map(|a| {
        Address::from_str(&a)
            .expect("valid signet address")
            .require_network(settings.network)
            .expect("correct network")
    });
    let alpen_address =
        alpen_address.map(|a| AlpenAddress::from_str(&a).expect("valid Alpen address"));

    if let Some(address) = signet_address {
        let mut l1w =
            SignetWallet::new(&seed, settings.network, settings.signet_backend.clone()).unwrap();
        l1w.sync().await.unwrap();
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
            builder.finish().expect("valid transaction")
        };
        l1w.sign(&mut psbt, Default::default()).unwrap();
        let tx = psbt.extract_tx().expect("fully signed tx");
        settings.signet_backend.broadcast_tx(&tx).await.unwrap();
        let txid = tx.compute_txid();
        println!(
            "{}",
            OnchainObject::from(&txid)
                .with_maybe_explorer(settings.mempool_space_endpoint.as_deref())
                .pretty()
        );
        println!("Drained signet wallet to {}", address,);
    }

    if let Some(address) = alpen_address {
        let l2w = AlpenWallet::new(&seed, &settings.alpen_endpoint).unwrap();
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        if balance == U256::ZERO {
            println!("No Alpen bitcoin to send");
        }

        let estimate_tx = l2w
            .transaction_request()
            .from(l2w.default_signer_address())
            .to(address)
            .value(U256::from(1));

        let gas_price = l2w.get_gas_price().await.unwrap();
        let gas_estimate = l2w.estimate_gas(estimate_tx).await.unwrap() as u128;

        let total_fee = gas_estimate * gas_price;
        let max_send_amount = balance.saturating_sub(U256::from(total_fee));

        let tx = l2w.transaction_request().to(address).value(max_send_amount);

        let res = l2w.send_transaction(tx).await.unwrap();

        println!(
            "{}",
            OnchainObject::from(res.tx_hash())
                .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
                .pretty()
        );

        println!(
            "Drained {} from Alpen wallet to {}",
            Amount::from_sat((max_send_amount / U256::from(SATS_TO_WEI)).wrapping_to()),
            address,
        );
    }
}
