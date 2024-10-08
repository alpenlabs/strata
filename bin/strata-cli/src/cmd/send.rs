use std::str::FromStr;

use alloy::{
    network::TransactionBuilder,
    primitives::{Address as StrataAddress, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use console::Term;

use crate::{
    net_type::{net_type_or_exit, NetworkType},
    seed::Seed,
    settings::Settings,
    signet::{fee_rate_or_crash, log_fee_rate, print_explorer_url, EsploraClient, SignetWallet},
    strata::StrataWallet,
};

/// Send some bitcoin from the internal wallet.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
pub struct SendArgs {
    /// either "signet" or "strata"
    #[argh(positional)]
    network_type: String,

    /// amount to send in sats
    #[argh(positional)]
    amount: u64,

    /// address to send to
    #[argh(positional)]
    address: String,

    /// override signet fee rate in sat/vbyte. must be >1
    #[argh(option)]
    fee_rate: Option<u64>,
}

pub async fn send(args: SendArgs, seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    let network_type = net_type_or_exit(&args.network_type, &term);

    match network_type {
        NetworkType::Signet => {
            let amount = Amount::from_sat(args.amount);
            let address = Address::from_str(&args.address)
                .expect("valid address")
                .require_network(settings.network)
                .expect("correct network");
            let mut l1w = SignetWallet::new(&seed, settings.network).expect("valid wallet");
            l1w.sync(&esplora).await.unwrap();
            let fee_rate = fee_rate_or_crash(args.fee_rate, &esplora).await;
            log_fee_rate(&term, &fee_rate);
            let mut psbt = l1w
                .build_tx()
                .add_recipient(address.script_pubkey(), amount)
                .fee_rate(fee_rate)
                .clone()
                .finish()
                .expect("valid psbt");
            l1w.sign(&mut psbt, Default::default())
                .expect("signable psbt");
            let tx = psbt.extract_tx().expect("signed tx");
            esplora.broadcast(&tx).await.expect("successful broadcast");
            let _ = print_explorer_url(&tx.compute_txid(), &term);
        }
        NetworkType::Strata => {
            let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).expect("valid wallet");
            let address = StrataAddress::from_str(&args.address).expect("valid address");
            let tx = TransactionRequest::default()
                .with_to(address)
                // 1 btc == 1 "eth" => 1 sat = 1e10 "wei"
                .with_value(U256::from(args.amount * 10u64.pow(10)));
            let res = l2w
                .send_transaction(tx)
                .await
                .expect("successful broadcast");
            let _ = term.write_line(&format!("Transaction {} sent", res.tx_hash()));
        }
    };

    let _ = term.write_line(&format!("Sent {} to {}", args.amount, args.address,));
}
