use std::sync::Arc;

use bitcoin::{
    secp256k1::{XOnlyPublicKey, SECP256K1},
    taproot::{LeafVersion, TaprootBuilder},
    Address, Amount, Network, OutPoint, TapNodeHash, Transaction, Txid,
};
use bitcoind::BitcoinD;
use common::bridge::{setup, BridgeDuty, User, MIN_FEE};
use strata_bridge_tx_builder::prelude::{
    create_tx, create_tx_ins, create_tx_outs, get_aggregated_pubkey, metadata_script,
    n_of_n_script, DepositInfo, BRIDGE_DENOMINATION, UNSPENDABLE_INTERNAL_KEY,
};
use strata_primitives::{bridge::PublickeyTable, buf::Buf20, l1::BitcoinAddress};
use tokio::sync::Mutex;
use tracing::{debug, event, info, span, Level};

mod common;

#[tokio::test]
async fn deposit_flow() {
    let num_operators = 5;

    let (bitcoind, federation) = setup(num_operators).await;

    // user creates the DRT
    let (txid, take_back_leaf_hash, taproot_addr, el_address) =
        perform_user_actions(federation.pubkey_table, bitcoind.clone()).await;

    // create `DepositInfo` from the DRT
    let deposit_info = perform_rollup_actions(
        txid,
        take_back_leaf_hash,
        taproot_addr,
        &el_address[..].try_into().unwrap(),
    );
    debug!(?deposit_info, event = "received deposit info");

    let duty = BridgeDuty::Deposit(deposit_info);

    let mut handles = Vec::new();
    for mut operator in federation.operators {
        let duty = duty.clone();
        handles.push(tokio::spawn(async move {
            operator.process_duty(duty.clone()).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    info!("Deposit flow complete");
}

async fn perform_user_actions(
    federation_pubkey_table: PublickeyTable,
    bitcoind: Arc<Mutex<BitcoinD>>,
) -> (Txid, TapNodeHash, Address, Vec<u8>) {
    let span = span!(Level::INFO, "user actions");
    let _guard = span.enter();

    let user = User::new("end-user", bitcoind.clone()).await;
    event!(Level::INFO, event = "User created", address = ?user.address());

    event!(Level::INFO, action = "sending funds to user's address");
    let balance = user.agent().mine_blocks(1).await;
    event!(Level::INFO, user_balance = %balance);

    assert!(
        balance.gt(&BRIDGE_DENOMINATION.into()),
        "user balance must be greater than the bridge denomination, got: {}, expected > {}",
        balance,
        BRIDGE_DENOMINATION
    );
    event!(Level::INFO, action = "getting available utxos");

    let (change_address, outpoint, amount) = user
        .agent()
        .select_utxo(BRIDGE_DENOMINATION.into())
        .await
        .expect("should get utxo with enough amount");
    event!(Level::INFO, event = "got change address and outpoint to use", change_address = %change_address, outpoint = %outpoint, amount = %amount);

    let (drt, take_back_leaf_hash, taproot_addr, el_address) = create_drt(
        outpoint,
        federation_pubkey_table,
        *UNSPENDABLE_INTERNAL_KEY,
        change_address,
        amount,
    );
    event!(Level::TRACE, event = "created DRT", drt = ?drt);

    event!(Level::INFO, action = "signing DRT with wallet");
    let signed_tx_result = user.agent().sign_raw_tx(&drt).await;
    assert!(signed_tx_result.complete, "tx should be fully signed");

    let signed_tx = signed_tx_result
        .transaction()
        .expect("should be able to get fully signed transaction");

    event!(Level::INFO, action = "broadcasting signed DRT");
    let txid = user.agent().broadcast_signed_tx(&signed_tx).await;
    event!(Level::INFO, event = "broadcasted signed DRT", txid = %txid);

    (txid, take_back_leaf_hash, taproot_addr, el_address)
}

fn create_drt(
    outpoint: OutPoint,
    pubkeys: PublickeyTable,
    internal_key: XOnlyPublicKey,
    change_address: Address,
    total_amt: Amount,
) -> (Transaction, TapNodeHash, Address, Vec<u8>) {
    let input = create_tx_ins([outpoint]);

    let (drt_addr, take_back_leaf_hash, el_address) =
        create_drt_taproot_output(pubkeys, internal_key);

    let output = create_tx_outs([
        (drt_addr.script_pubkey(), BRIDGE_DENOMINATION.into()),
        (
            change_address.script_pubkey(),
            total_amt - BRIDGE_DENOMINATION.into() - MIN_FEE,
        ),
    ]);

    (
        create_tx(input, output),
        take_back_leaf_hash,
        drt_addr,
        el_address,
    )
}

fn create_drt_taproot_output(
    pubkeys: PublickeyTable,
    internal_key: XOnlyPublicKey,
) -> (Address, TapNodeHash, Vec<u8>) {
    let aggregated_pubkey = get_aggregated_pubkey(pubkeys);
    let n_of_n_spend_script = n_of_n_script(&aggregated_pubkey);

    // in actual DRT, this will be the take-back leaf.
    // for testing, this could be any script as we only care about its hash.
    let el_address = Buf20::default().0 .0;
    let op_return_script = metadata_script(&el_address[..].try_into().unwrap());
    let op_return_script_hash = TapNodeHash::from_script(&op_return_script, LeafVersion::TapScript);

    let taproot_builder = TaprootBuilder::new()
        .add_leaf(1, n_of_n_spend_script.clone())
        .unwrap()
        .add_leaf(1, op_return_script)
        .unwrap();

    let spend_info = taproot_builder.finalize(SECP256K1, internal_key).unwrap();

    (
        Address::p2tr(
            SECP256K1,
            internal_key,
            spend_info.merkle_root(),
            Network::Regtest,
        ),
        op_return_script_hash,
        el_address.to_vec(),
    )
}

fn perform_rollup_actions(
    txid: Txid,
    take_back_leaf_hash: TapNodeHash,
    original_taproot_addr: Address,
    el_address: &[u8; 20],
) -> DepositInfo {
    let span = span!(Level::INFO, "rollup actions");
    let _guard = span.enter();

    let deposit_request_outpoint = OutPoint { txid, vout: 0 };
    let total_amount: Amount = BRIDGE_DENOMINATION.into();
    let original_taproot_addr = BitcoinAddress::new(original_taproot_addr.as_unchecked().clone());

    event!(Level::INFO, action = "creating deposit info");
    DepositInfo::new(
        deposit_request_outpoint,
        el_address.to_vec(),
        total_amount,
        take_back_leaf_hash,
        original_taproot_addr,
    )
}
