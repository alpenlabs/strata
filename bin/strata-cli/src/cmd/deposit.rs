use std::{str::FromStr, time::Duration};

use alloy::{primitives::Address as StrataAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{
    bitcoin::{hashes::Hash, taproot::LeafVersion, Address, TapNodeHash, XOnlyPublicKey},
    chain::ChainOracle,
    descriptor::IntoWalletDescriptor,
    miniscript::{miniscript::Tap, Miniscript},
    template::DescriptorTemplateOut,
    KeychainKind, TxOrdering, Wallet,
};
use colored::Colorize;
use indicatif::ProgressBar;
use strata_primitives::constants::UNSPENDABLE_PUBLIC_KEY;

use crate::{
    constants::{BRIDGE_IN_AMOUNT, RECOVER_AT_DELAY, RECOVER_DELAY, SIGNET_BLOCK_TIME},
    link::{OnchainObject, PrettyPrint},
    recovery::DescriptorRecovery,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, SignetWallet},
    strata::StrataWallet,
    taproot::{ExtractP2trPubkey, NotTaprootAddress},
};

/// Magic bytes to attach to the deposit request.
pub const MAGIC_BYTES: &[u8] = r"alpen".as_bytes();

/// Deposit 10 BTC from signet to Strata. If an address is not provided, the wallet's internal
/// Strata address will be used.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "deposit")]
pub struct DepositArgs {
    #[argh(positional)]
    strata_address: Option<String>,

    /// override signet fee rate in sat/vbyte. must be >=1
    #[argh(option)]
    fee_rate: Option<u64>,
}

pub async fn deposit(
    DepositArgs {
        strata_address,
        fee_rate,
    }: DepositArgs,
    seed: Seed,
    settings: Settings,
) {
    let requested_strata_address =
        strata_address.map(|a| StrataAddress::from_str(&a).expect("bad strata address"));
    let mut l1w =
        SignetWallet::new(&seed, settings.network, settings.signet_backend.clone()).unwrap();
    let l2w = StrataWallet::new(&seed, &settings.strata_endpoint).unwrap();

    l1w.sync().await.unwrap();
    let recovery_address = l1w.reveal_next_address(KeychainKind::External).address;
    l1w.persist().unwrap();

    let strata_address = requested_strata_address.unwrap_or(l2w.default_signer_address());
    println!(
        "Bridging {} to Strata address {}",
        BRIDGE_IN_AMOUNT.to_string().green(),
        strata_address.to_string().cyan(),
    );

    println!(
        "Recovery address: {}",
        recovery_address.to_string().yellow()
    );

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

    let recover_at = current_block_height + RECOVER_AT_DELAY;

    let bridge_in_address = temp_wallet
        .reveal_next_address(KeychainKind::External)
        .address;

    println!(
        "Using {} as bridge in address",
        bridge_in_address.to_string().yellow()
    );

    let fee_rate = get_fee_rate(fee_rate, settings.signet_backend.as_ref()).await;
    log_fee_rate(&fee_rate);

    const MBL: usize = MAGIC_BYTES.len();
    const TNHL: usize = TapNodeHash::LEN;
    let mut op_return_data = [0u8; MBL + TNHL + StrataAddress::len_bytes()];
    op_return_data[..MBL].copy_from_slice(MAGIC_BYTES);
    op_return_data[MBL..MBL + TNHL]
        .copy_from_slice(recovery_script_hash.as_raw_hash().as_byte_array());
    op_return_data[MBL + TNHL..].copy_from_slice(strata_address.as_slice());

    let mut psbt = {
        let mut builder = l1w.build_tx();
        // Important: the deposit won't be found by the sequencer if the order isn't correct.
        builder.ordering(TxOrdering::Untouched);
        builder.add_recipient(bridge_in_address.script_pubkey(), BRIDGE_IN_AMOUNT);
        builder.add_data(&op_return_data);
        builder.fee_rate(fee_rate);
        builder.finish().expect("valid psbt")
    };
    l1w.sign(&mut psbt, Default::default()).unwrap();
    println!("Built transaction");

    let tx = psbt.extract_tx().expect("valid tx");

    let pb = ProgressBar::new_spinner().with_message("Saving output descriptor");
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut desc_file = DescriptorRecovery::open(&seed, &settings.descriptor_db)
        .await
        .unwrap();
    desc_file
        .add_desc(recover_at, &bridge_in_desc)
        .await
        .unwrap();
    pb.finish_with_message("Saved output descriptor");

    let pb = ProgressBar::new_spinner().with_message("Broadcasting transaction");
    pb.enable_steady_tick(Duration::from_millis(100));
    settings
        .signet_backend
        .broadcast_tx(&tx)
        .await
        .expect("successful broadcast");
    let txid = tx.compute_txid();
    pb.finish_with_message(
        OnchainObject::from(&txid)
            .with_maybe_explorer(settings.mempool_space_endpoint.as_deref())
            .pretty(),
    );
    println!("Expect transaction confirmation in ~{SIGNET_BLOCK_TIME:?}. Funds will take longer than this to be available on Strata.");
}

fn bridge_in_descriptor(
    bridge_pubkey: XOnlyPublicKey,
    recovery_address: Address,
) -> Result<(DescriptorTemplateOut, TapNodeHash), NotTaprootAddress> {
    let recovery_xonly_pubkey = recovery_address.extract_p2tr_pubkey()?;

    let desc = bdk_wallet::descriptor!(
        tr(UNSPENDABLE_PUBLIC_KEY, {
            pk(bridge_pubkey),
            and_v(v:pk(recovery_xonly_pubkey),older(RECOVER_DELAY))
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
