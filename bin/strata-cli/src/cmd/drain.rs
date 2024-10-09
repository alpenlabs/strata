use std::str::FromStr;

use alloy::{
    primitives::{Address as StrataAddress, U256},
    providers::{utils::Eip1559Estimation, Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use console::{style, Term};

use crate::{
    seed::Seed,
    settings::Settings,
    signet::{fee_rate_or_crash, log_fee_rate, print_explorer_url, EsploraClient, SignetWallet},
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
    esplora: EsploraClient,
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
        let mut l1w = SignetWallet::new(&seed, settings.network).unwrap();
        l1w.sync(&esplora).await.unwrap();
        let balance = l1w.balance();
        if balance.untrusted_pending > Amount::ZERO {
            let _ = term.write_line(
                &style("You have pending funds on signet that won't be included in the drain")
                    .yellow()
                    .to_string(),
            );
        }
        let fr = fee_rate_or_crash(fee_rate, &esplora).await;
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
        esplora.broadcast(&tx).await.unwrap();
        let _ = print_explorer_url(&tx.compute_txid(), &term);
    }

    if let Some(address) = strata_address {
        let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        if balance == U256::ZERO {
            let _ = term.write_line("No Strata bitcoin to send");
        }
        let Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = l2w.estimate_eip1559_fees(None).await.unwrap();

        let estimate_tx = l2w
            .transaction_request()
            .to(address)
            .value(U256::from(1))
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas);

        let gas_limit = l2w.estimate_gas(&estimate_tx).await.unwrap();

        let max_gas_fee = gas_limit * max_fee_per_gas;
        let max_send_amount = balance.saturating_sub(U256::from(max_gas_fee));

        let tx = l2w
            .transaction_request()
            .to(address)
            .value(max_send_amount)
            .gas_limit(gas_limit)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas);

        let _ = l2w.send_transaction(tx).await.unwrap();
    }
}
