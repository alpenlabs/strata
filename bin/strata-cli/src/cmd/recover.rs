use argh::FromArgs;
use bdk_esplora::EsploraAsyncExt;
use bdk_wallet::{
    bitcoin::Amount, chain::ChainOracle, descriptor::IntoWalletDescriptor, KeychainKind, Wallet,
};
use console::{style, Term};

use crate::{
    constants::NETWORK,
    recovery::DescriptorRecovery,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, EsploraClient, SignetWallet},
};

/// Attempt recovery of old bridge-in transactions
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "recover")]
pub struct RecoverArgs {}

pub async fn recover(seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    let mut l1w = SignetWallet::new(&seed, NETWORK).unwrap();
    l1w.sync(&esplora).await.unwrap();

    let _ = term.write_line("Opening descriptor recovery");
    let mut descriptor_file = DescriptorRecovery::open(&seed, &settings.descriptor_db)
        .await
        .unwrap();
    let current_height = l1w.local_chain().get_chain_tip().unwrap().height;
    let _ = term.write_line(&format!("Current signet chain height: {current_height}"));
    let descs = descriptor_file
        .read_descs_after_block(current_height)
        .await
        .unwrap();

    if descs.is_empty() {
        let _ = term.write_line("Nothing to recover");
        return;
    }

    let fee_rate = get_fee_rate(1, &esplora)
        .await
        .expect("request should succeed")
        .expect("valid target");

    for desc in descs {
        let desc = desc
            .clone()
            .into_wallet_descriptor(l1w.secp_ctx(), NETWORK)
            .expect("valid descriptor");

        let mut recovery_wallet = Wallet::create_single(desc)
            .network(NETWORK)
            .create_wallet_no_persist()
            .expect("valid wallet");

        // reveal the address for the wallet so we can sync it
        let address = recovery_wallet.reveal_next_address(KeychainKind::External);
        let req = recovery_wallet.start_sync_with_revealed_spks().build();
        let update = esplora.sync(req, 3).await.unwrap();
        recovery_wallet.apply_update(update).unwrap();
        let needs_recovery = recovery_wallet.balance().confirmed > Amount::ZERO;

        if !needs_recovery {
            continue;
        }

        recovery_wallet.transactions().for_each(|tx| {
            l1w.insert_tx(tx.tx_node.tx);
        });

        let recover_to = l1w.reveal_next_address(KeychainKind::External).address;
        let _ = term.write_line(&format!(
            "Recovering a bridge-in transaction from address {} to {}",
            style(address).yellow(),
            style(&recover_to).yellow()
        ));

        // we want to drain the recovery path to the l1 wallet
        let mut psbt = recovery_wallet
            .build_tx()
            .drain_to(recover_to.script_pubkey())
            .fee_rate(fee_rate)
            .clone()
            .finish()
            .expect("valid tx");

        recovery_wallet
            .sign(&mut psbt, Default::default())
            .expect("valid sign op");

        let tx = psbt.extract_tx().unwrap();
        esplora
            .broadcast(&tx)
            .await
            .expect("successful tx broadcast");
    }
}
