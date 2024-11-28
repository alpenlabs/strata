use std::{str::FromStr, time::Duration};

use alloy::{
    network::TransactionBuilder, primitives::U256, providers::Provider,
    rpc::types::TransactionInput,
};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use console::Term;
use indicatif::ProgressBar;

use crate::{
    constants::{BRIDGE_OUT_AMOUNT, SATS_TO_WEI},
    link::{OnchainObject, PrettyPrint},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
    strata::StrataWallet,
    taproot::ExtractP2trPubkey,
};

/// Withdraw 10 BTC from Strata to signet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "withdraw")]
pub struct WithdrawArgs {
    /// the signet address to send funds to. defaults to a new internal wallet address
    #[argh(positional)]
    p2tr_address: Option<String>,
}

pub async fn withdraw(args: WithdrawArgs, seed: Seed, settings: Settings) {
    let address = args.p2tr_address.map(|a| {
        Address::from_str(&a)
            .expect("valid address")
            .require_network(settings.network)
            .expect("correct network")
    });

    let mut l1w =
        SignetWallet::new(&seed, settings.network, settings.signet_backend.clone()).unwrap();
    let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();

    let address = match address {
        Some(a) => a,
        None => {
            let info = l1w.reveal_next_address(KeychainKind::External);
            l1w.persist().unwrap();
            info.address
        }
    };

    let term = Term::stdout();
    let _ = term.write_line(&format!(
        "Bridging out {} to {}",
        BRIDGE_OUT_AMOUNT, address
    ));

    let tx = l2w
        .transaction_request()
        .with_to(settings.bridge_strata_address)
        .with_value(U256::from(BRIDGE_OUT_AMOUNT.to_sat() as u128 * SATS_TO_WEI))
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
    pb.finish_with_message("Broadcast successful");
    let _ = term.write_line(
        &OnchainObject::from(res.tx_hash())
            .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
            .pretty(),
    );
}
