use std::str::FromStr;

use alloy::{
    primitives::{Address as RollupAddress, U256},
    providers::{utils::Eip1559Estimation, Provider, WalletProvider},
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use console::{style, Term};

use crate::{
    rollup::RollupWallet,
    seed::Seed,
    settings::SETTINGS,
    signet::{get_fee_rate, log_fee_rate, SignetWallet, ESPLORA_CLIENT},
};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "drain")]
/// Drains the internal wallet to the provided
/// signet and rollup addresses
pub struct DrainArgs {
    #[argh(option, short = 's')]
    /// an optional signet address for signet funds to be drained to
    signet_address: Option<String>,
    #[argh(option, short = 'r')]
    /// an optional rollup address for rollup funds to be drained to
    rollup_address: Option<String>,
}

pub async fn drain(
    DrainArgs {
        signet_address,
        rollup_address,
    }: DrainArgs,
    seed: Seed,
) {
    let term = Term::stdout();
    if rollup_address.is_none() && signet_address.is_none() {
        let _ = term.write_line("Either signet or rollup address should be provided");
    }

    let signet_address = signet_address.map(|a| {
        Address::from_str(&a)
            .expect("valid signet address")
            .require_network(SETTINGS.network)
            .expect("correct network")
    });
    let rollup_address =
        rollup_address.map(|a| RollupAddress::from_str(&a).expect("valid rollup address"));

    if let Some(address) = signet_address {
        let mut l1w = SignetWallet::new(&seed).unwrap();
        l1w.sync().await.unwrap();
        let balance = l1w.balance();
        if balance.untrusted_pending > Amount::ZERO {
            let _ = term.write_line(
                &style("You have pending, untrusted funds on signet that won't be included in the drain")
                    .yellow().to_string()
            );
        }
        let fr = get_fee_rate(1).await.unwrap().unwrap();
        log_fee_rate(&term, &fr);
        let mut psbt = l1w
            .build_tx()
            .drain_to(address.script_pubkey())
            .fee_rate(fr)
            .clone()
            .finish()
            .expect("valid transaction");
        l1w.sign(&mut psbt, Default::default()).unwrap();
        let tx = psbt.extract_tx().expect("fully signed tx");
        ESPLORA_CLIENT.broadcast(&tx).await.unwrap();
    }

    if let Some(address) = rollup_address {
        let l2w = RollupWallet::new(&seed).unwrap();
        let balance = l2w.get_balance(l2w.default_signer_address()).await.unwrap();
        if balance == U256::ZERO {
            let _ = term.write_line("No rollup funds to send");
        }
        let Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = l2w.estimate_eip1559_fees(None).await.unwrap();

        let estimate_tx = l2w
            .transaction_request()
            .to(address)
            .value(balance) // Use full balance for estimation
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
