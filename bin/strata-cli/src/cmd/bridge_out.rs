use std::{str::FromStr, time::Duration};

use alloy::{
    network::TransactionBuilder, primitives::U256, providers::Provider,
    rpc::types::TransactionInput,
};
use argh::FromArgs;
use bdk_wallet::{
    bitcoin::{Address, Amount},
    KeychainKind,
};
use console::Term;
use indicatif::ProgressBar;

use crate::{
    rollup::RollupWallet, seed::Seed, settings::Settings, signet::SignetWallet,
    taproot::ExtractP2trPubkey,
};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "bridge-out")]
/// Bridge 10 BTC from the rollup to signet
pub struct BridgeOutArgs {
    #[argh(positional)]
    p2tr_address: Option<String>,
}

pub async fn bridge_out(args: BridgeOutArgs, seed: Seed, settings: Settings) {
    let address = args.p2tr_address.map(|a| {
        Address::from_str(&a)
            .expect("valid address")
            .require_network(settings.network)
            .expect("correct network")
    });

    let mut l1w = SignetWallet::new(&seed).unwrap();
    let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).unwrap();

    let address = match address {
        Some(a) => a,
        None => {
            let info = l1w.reveal_next_address(KeychainKind::External);
            l1w.persist().unwrap();
            info.address
        }
    };

    const AMOUNT: Amount = Amount::from_int_btc(10);

    let term = Term::stdout();
    let _ = term.write_line(&format!("Bridging out {} to {}", AMOUNT, address));

    let tx = l2w
        .transaction_request()
        .with_to(settings.bridge_rollup_address)
        .with_value(U256::from(AMOUNT.to_sat() * 1u64.pow(10)))
        .input(TransactionInput::new(
            address
                .extract_p2tr_pubkey()
                .expect("valid P2TR address")
                .serialize()
                .into(),
        ));

    let pb = ProgressBar::new_spinner().with_message("Broadcasting transaction");
    pb.enable_steady_tick(Duration::from_millis(100));
    let res = l2w.send_transaction(tx).await.unwrap();
    let txid = res.tx_hash();
    pb.finish_with_message(format!("Broadcast successful. Txid: {}", txid));
}
