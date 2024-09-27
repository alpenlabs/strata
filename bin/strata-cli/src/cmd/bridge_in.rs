use std::{str::FromStr, time::Duration};

use alloy::{primitives::Address as RollupAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{
    bitcoin::{hashes::Hash, taproot::LeafVersion, Address, Amount, TapNodeHash, XOnlyPublicKey},
    chain::ChainOracle,
    descriptor::IntoWalletDescriptor,
    miniscript::{miniscript::Tap, Miniscript},
    template::DescriptorTemplateOut,
    KeychainKind, TxOrdering, Wallet,
};
use console::{style, Term};
use indicatif::ProgressBar;

use crate::{
    recovery::DescriptorRecovery,
    rollup::RollupWallet,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, EsploraClient, SignetWallet},
    taproot::{ExtractP2trPubkey, NotTaprootAddress, UNSPENDABLE},
};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "bridge-in")]
/// Bridge 10 BTC from signet to the rollup. If an address is not provided, the wallet's internal
/// rollup address will be used.
pub struct BridgeInArgs {
    #[argh(positional)]
    rollup_address: Option<String>,
}

pub async fn bridge_in(args: BridgeInArgs, seed: Seed, settings: Settings, esplora: EsploraClient) {
    let term = Term::stdout();
    let requested_rollup_address = args
        .rollup_address
        .map(|a| RollupAddress::from_str(&a).expect("bad rollup address"));
    let mut l1w = SignetWallet::new(&seed).unwrap();
    let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).unwrap();

    l1w.sync(&esplora).await.unwrap();
    let recovery_address = l1w.reveal_next_address(KeychainKind::External).address;
    l1w.persist().unwrap();

    let rollup_address = requested_rollup_address.unwrap_or(l2w.default_signer_address());
    const AMOUNT: Amount = Amount::from_sat(1_001_000_000); // 10.01 BTC
    let _ = term.write_line(&format!(
        "Bridging {} to rollup address {}",
        style(AMOUNT.to_string()).green(),
        style(rollup_address).cyan(),
    ));

    let _ = term.write_line(&format!(
        "Recovery address: {}",
        style(recovery_address.to_string()).yellow()
    ));

    let (bridge_in_desc, recovery_script_hash) =
        bridge_in_descriptor(settings.bridge_musig2_pubkey, recovery_address)
            .expect("valid bridge in descriptor");

    let desc = bridge_in_desc
        .clone()
        .into_wallet_descriptor(l1w.secp_ctx(), settings.network)
        .expect("valid descriptor");

    let mut temp_wallet = Wallet::create_single(desc.clone())
        .network(settings.network)
        .create_wallet_no_persist()
        .expect("valid wallet");

    let current_block_height = l1w
        .local_chain()
        .get_chain_tip()
        .expect("valid chain tip")
        .height;

    let recover_at = current_block_height + 1050;

    let bridge_in_address = temp_wallet
        .reveal_next_address(KeychainKind::External)
        .address;

    let _ = term.write_line(&format!(
        "Using {} as bridge in address",
        style(bridge_in_address.to_string()).yellow()
    ));

    let fee_rate = get_fee_rate(1, &esplora)
        .await
        .expect("should get fee rate")
        .expect("should have valid fee rate");

    log_fee_rate(&term, &fee_rate);

    let mut op_return_data = [0u8; 11 + 32 + 20];
    op_return_data[..11].copy_from_slice(b"alpenstrata");
    op_return_data[11..11 + 32].copy_from_slice(recovery_script_hash.as_raw_hash().as_byte_array());
    op_return_data[11 + 32..].copy_from_slice(rollup_address.as_slice());

    let mut psbt = l1w
        .build_tx()
        .ordering(TxOrdering::Untouched)
        .add_recipient(bridge_in_address.script_pubkey(), AMOUNT)
        .add_data(&op_return_data)
        .enable_rbf()
        .fee_rate(fee_rate)
        .clone()
        .finish()
        .expect("valid psbt");
    l1w.sign(&mut psbt, Default::default()).unwrap();
    let _ = term.write_line("Built transaction");

    let tx = psbt.extract_tx().expect("valid tx");

    let pb = ProgressBar::new_spinner().with_message("Saving output descriptor");
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut desc_file = DescriptorRecovery::open(&seed, &settings.descriptor_db)
        .await
        .unwrap();
    desc_file
        .add_desc(
            recover_at,
            &bridge_in_desc,
            l1w.secp_ctx(),
            settings.network,
        )
        .await
        .unwrap();
    pb.finish_with_message("Saved output descriptor");

    let pb = ProgressBar::new_spinner().with_message("Broadcasting transaction");
    pb.enable_steady_tick(Duration::from_millis(100));
    esplora.broadcast(&tx).await.expect("successful broadcast");
    pb.finish_with_message(format!("Transaction {} broadcasted", tx.compute_txid()));
    let _ = term.write_line(&format!(
        "Expect transaction confirmation in ~{:?}. Funds will take longer than this to be available on rollup.",
        settings.block_time
    ));
}

fn bridge_in_descriptor(
    bridge_pubkey: XOnlyPublicKey,
    recovery_address: Address,
) -> Result<(DescriptorTemplateOut, TapNodeHash), NotTaprootAddress> {
    let recovery_xonly_pubkey = recovery_address.extract_p2tr_pubkey()?;

    let desc = bdk_wallet::descriptor!(
        tr(UNSPENDABLE, {
            pk(bridge_pubkey),
            and_v(v:pk(recovery_xonly_pubkey),older(1008))
        })
    )
    .expect("valid descriptor");

    // we have to do this to obtain the script hash
    // i have tried to extract it directly from the desc above
    // it is a massive pita
    let recovery_script = Miniscript::<XOnlyPublicKey, Tap>::from_str(&format!(
        "and_v(v:pk({}),older(1008))",
        recovery_xonly_pubkey
    ))
    .expect("valid recovery script")
    .encode();

    let recovery_script_hash = TapNodeHash::from_script(&recovery_script, LeafVersion::TapScript);

    Ok((desc, recovery_script_hash))
}
