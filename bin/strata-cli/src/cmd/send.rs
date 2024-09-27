use std::str::FromStr;

use alloy::{
    network::TransactionBuilder,
    primitives::{Address as RollupAddress, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{hashes::Hash, Address, Amount};
use console::Term;
use hex::encode;

use crate::{
    rollup::RollupWallet,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, EsploraClient, SignetWallet},
};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
/// Send some bitcoin from the internal wallet.
pub struct SendArgs {
    #[argh(switch)]
    /// send via signet
    signet: bool,
    #[argh(switch)]
    /// send via rollup
    rollup: bool,
    #[argh(positional)]
    /// amount to send in sats
    amount: u64,
    #[argh(positional)]
    address: String,
}

pub async fn send(args: SendArgs, seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    if args.signet && args.rollup {
        let _ = term.write_line("Cannot use both --signet and --rollup options at once");
        std::process::exit(1);
    } else if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    let txid = if args.signet {
        let amount = Amount::from_sat(args.amount);
        let address = Address::from_str(&args.address)
            .expect("valid address")
            .require_network(settings.network)
            .expect("correct network");
        let mut l1w = SignetWallet::new(&seed).unwrap();
        l1w.sync(&esplora).await.unwrap();
        let fee_rate = get_fee_rate(1, &esplora).await.unwrap().unwrap();
        let mut psbt = l1w
            .build_tx()
            .add_recipient(address.script_pubkey(), amount)
            .enable_rbf()
            .fee_rate(fee_rate)
            .clone()
            .finish()
            .unwrap();
        l1w.sign(&mut psbt, Default::default()).unwrap();
        let tx = psbt.extract_tx().unwrap();
        esplora.broadcast(&tx).await.unwrap();
        tx.compute_txid().as_raw_hash().to_byte_array()
    } else if args.rollup {
        let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).unwrap();
        let address = RollupAddress::from_str(&args.address).expect("valid address");
        let tx = TransactionRequest::default()
            .with_to(address)
            // 1 btc == 1 "eth" => 1 sat = 1e10 "wei"
            .with_value(U256::from(args.amount * 10u64.pow(10)));
        l2w.send_transaction(tx).await.unwrap().tx_hash().0
    } else {
        unreachable!()
    };

    let _ = term.write_line(&format!(
        "Sent {} to {} in tx {}",
        args.amount,
        args.address,
        encode(&txid)
    ));
}
