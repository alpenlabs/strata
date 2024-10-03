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
use shrex::encode;

use crate::{
    constants::NETWORK,
    rollup::RollupWallet,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, EsploraClient, SignetWallet},
};

/// Send some bitcoin from the internal wallet.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
pub struct SendArgs {
    /// either "signet" or "rollup"
    #[argh(positional)]
    network_type: String,

    /// amount to send in sats
    #[argh(positional)]
    amount: u64,

    /// address to send to
    #[argh(positional)]
    address: String,
}

enum NetworkType {
    Signet,
    Rollup,
}

struct InvalidNetworkType;

impl FromStr for NetworkType {
    type Err = InvalidNetworkType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "rollup" => Ok(Self::Rollup),
            _ => Err(InvalidNetworkType),
        }
    }
}

pub async fn send(args: SendArgs, seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    let network_type = match NetworkType::from_str(&args.network_type) {
        Ok(t) => t,
        Err(InvalidNetworkType) => {
            let _ = term.write_line("Invalid network type. Must be signet or rollup");
            return;
        }
    };

    let txid = match network_type {
        NetworkType::Signet => {
            let amount = Amount::from_sat(args.amount);
            let address = Address::from_str(&args.address)
                .expect("valid address")
                .require_network(NETWORK)
                .expect("correct network");
            let mut l1w = SignetWallet::new(&seed, NETWORK).expect("valid wallet");
            l1w.sync(&esplora).await.unwrap();
            let fee_rate = get_fee_rate(1, &esplora)
                .await
                .expect("valid response")
                .expect("valid target");
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
            tx.compute_txid().as_raw_hash().to_byte_array()
        }
        NetworkType::Rollup => {
            let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).expect("valid wallet");
            let address = RollupAddress::from_str(&args.address).expect("valid address");
            let tx = TransactionRequest::default()
                .with_to(address)
                // 1 btc == 1 "eth" => 1 sat = 1e10 "wei"
                .with_value(U256::from(args.amount * 10u64.pow(10)));
            l2w.send_transaction(tx)
                .await
                .expect("successful broadcast")
                .tx_hash()
                .0
        }
    };

    let _ = term.write_line(&format!(
        "Sent {} to {} in tx {}",
        args.amount,
        args.address,
        encode(&txid)
    ));
}
