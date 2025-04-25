use std::str::FromStr;

use alloy::{
    network::TransactionBuilder,
    primitives::{Address as AlpenAddress, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};

use crate::{
    alpen::AlpenWallet,
    constants::SATS_TO_WEI,
    link::{OnchainObject, PrettyPrint},
    net_type::{net_type_or_exit, NetworkType},
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, SignetWallet},
};

/// Send some bitcoin from the internal wallet.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
pub struct SendArgs {
    /// either "signet" or "alpen"
    #[argh(positional)]
    network_type: String,

    /// amount to send in sats
    #[argh(positional)]
    amount: u64,

    /// address to send to
    #[argh(positional)]
    address: String,

    /// override signet fee rate in sat/vbyte. must be >=1
    #[argh(option)]
    fee_rate: Option<u64>,
}

pub async fn send(args: SendArgs, seed: Seed, settings: Settings) {
    let network_type = net_type_or_exit(&args.network_type);

    match network_type {
        NetworkType::Signet => {
            let amount = Amount::from_sat(args.amount);
            let address = Address::from_str(&args.address)
                .unwrap_or_else(|_| {
                    eprintln!("Invalid signet address provided as argument.");
                    std::process::exit(1);
                })
                .require_network(settings.network)
                .expect("correct network");
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .expect("valid wallet");
            l1w.sync().await.unwrap();
            let fee_rate = get_fee_rate(args.fee_rate, settings.signet_backend.as_ref()).await;
            log_fee_rate(&fee_rate);
            let mut psbt = {
                let mut builder = l1w.build_tx();
                builder.add_recipient(address.script_pubkey(), amount);
                builder.fee_rate(fee_rate);
                builder.finish().expect("valid psbt")
            };
            l1w.sign(&mut psbt, Default::default())
                .expect("signable psbt");
            let tx = psbt.extract_tx().expect("signed tx");
            settings
                .signet_backend
                .broadcast_tx(&tx)
                .await
                .expect("successful broadcast");
            let txid = tx.compute_txid();
            println!(
                "{}",
                OnchainObject::from(&txid)
                    .with_maybe_explorer(settings.mempool_space_endpoint.as_deref())
                    .pretty(),
            );
        }
        NetworkType::Alpen => {
            let l2w = AlpenWallet::new(&seed, &settings.alpen_endpoint).expect("valid wallet");
            let address = AlpenAddress::from_str(&args.address).unwrap_or_else(|_| {
                eprintln!(
                    "Invalid strata address provided as argument - must be an EVM-compatible address."
                );
                std::process::exit(1);
            });
            let tx = TransactionRequest::default()
                .with_to(address)
                .with_value(U256::from(args.amount as u128 * SATS_TO_WEI));
            let res = l2w
                .send_transaction(tx)
                .await
                .expect("successful broadcast");
            println!(
                "{}",
                OnchainObject::from(res.tx_hash())
                    .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
                    .pretty(),
            );
        }
    };

    println!("Sent {} to {}", Amount::from_sat(args.amount), args.address,);
}
