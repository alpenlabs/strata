use std::str::FromStr;

use alloy::{
    primitives::{Address as StrataAddress, U256},
    providers::{Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use console::{style, Term};

use crate::{
    constants::SATS_TO_WEI,
    seed::Seed,
    settings::Settings,
    signet::{broadcast_tx, get_fee_rate, log_fee_rate, print_explorer_url, SignetWallet},
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
) {
    let term = Term::stdout();
    if strata_address.is_none() && signet_address.is_none() {
        let _ = term.write_line("Either signet or Strata address should be provided");
    }

    let signet_address = signet_address.map(|a| {
        Address::from_str(&a)
            .expect("valid signet address")
            .require_network(settings.network)
            .expect("correct network")
    });
    let strata_address =
        strata_address.map(|a| StrataAddress::from_str(&a).expect("valid Strata address"));

    if let Some(address) = signet_address {
        let mut l1w =
            SignetWallet::new(&seed, settings.network, settings.sync_backend.clone()).unwrap();
        l1w.sync().await.unwrap();
        let balance = l1w.balance();
        if balance.untrusted_pending > Amount::ZERO {
            let _ = term.write_line(
                &style("You have pending funds on signet that won't be included in the drain")
                    .yellow()
                    .to_string(),
            );
        }
        let fr = get_fee_rate(fee_rate, settings.sync_backend.clone(), 1)
            .await
            .expect("valid fee rate");
        log_fee_rate(&term, &fr);

        let mut psbt = l1w
            .build_tx()
            .drain_wallet()
            .drain_to(address.script_pubkey())
            .fee_rate(fr)
            .clone()
            .finish()
            .expect("valid transaction");
        l1w.sign(&mut psbt, Default::default()).unwrap();
        let tx = psbt.extract_tx().expect("fully signed tx");
        broadcast_tx(&tx, settings.sync_backend.clone())
            .await
            .unwrap();
        let _ = print_explorer_url(&tx.compute_txid(), &term, &settings);
        let _ = term.write_line(&format!("Drained signet wallet to {}", address,));
    }

    if let Some(address) = strata_address {
        let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        if balance == U256::ZERO {
            let _ = term.write_line("No Strata bitcoin to send");
        }

        let estimate_tx = l2w
            .transaction_request()
            .from(l2w.default_signer_address())
            .to(address)
            .value(U256::from(1));

        let gas_price = l2w.get_gas_price().await.unwrap();
        let gas_estimate = l2w.estimate_gas(&estimate_tx).await.unwrap();

        let total_fee = gas_estimate * gas_price;
        let max_send_amount = balance.saturating_sub(U256::from(total_fee));

        let tx = l2w.transaction_request().to(address).value(max_send_amount);

        let _ = l2w.send_transaction(tx).await.unwrap();

        let _ = term.write_line(&format!(
            "Drained {} from Strata wallet to {}",
            Amount::from_sat((max_send_amount / U256::from(SATS_TO_WEI)).wrapping_to()),
            address,
        ));
    }
}
