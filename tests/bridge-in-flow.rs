//! Tests the bridge-in flow.
//!
//! This is done by creating the Deposit Request Transaction, manually creating a `DepositInfo` out
//! of it and calling appropriate methods to create the final Deposit Transaction.

use std::sync::Arc;

use bitcoincore_rpc::{
    bitcoin::{Amount, Network},
    json::ScanTxOutRequest,
    Client, RpcApi,
};
use common::bridge::{perform_rollup_actions, perform_user_actions, setup, BridgeDuty, User};
use strata_bridge_tx_builder::prelude::{
    create_taproot_addr, get_aggregated_pubkey, SpendPath, BRIDGE_DENOMINATION,
};
use strata_primitives::bridge::PublickeyTable;
use tracing::{debug, event, info, Level};

mod common;

#[tokio::test]
async fn deposit_flow() {
    let num_operators = 5;

    let (bitcoind, client, federation) = setup(num_operators).await;

    let pubkey_table = federation.pubkey_table.clone();

    // user creates the DRT
    let user = User::new("end-user", bitcoind.clone()).await;
    event!(Level::INFO, event = "User created", address = ?user.address());

    let (txid, take_back_leaf_hash, taproot_addr, el_address) =
        perform_user_actions(&user, federation.pubkey_table).await;

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
            operator.process_duty(duty).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    confirm_deposit(client.clone(), &user, pubkey_table).await;

    info!("Deposit flow complete");
}

async fn confirm_deposit(client: Arc<Client>, user: &User, pubkey_table: PublickeyTable) {
    let num_blocks = 1;
    event!(Level::DEBUG, action = "mining some blocks to confirm deposit transaction", num_blocks = %num_blocks);

    user.agent().mine_blocks(num_blocks).await;

    let (bridge_addr, _) = create_taproot_addr(
        &Network::Regtest,
        SpendPath::KeySpend {
            internal_key: get_aggregated_pubkey(pubkey_table),
        },
    )
    .expect("should be able to compute the bridge taproot address");

    let bridge_script_pubkey = bridge_addr.script_pubkey();

    event!(Level::DEBUG, action = "scanning tx outs in the bridge address", bridge_addr=?bridge_addr);
    let utxos = client
        .scan_tx_out_set_blocking(&[ScanTxOutRequest::Single(format!(
            "raw({})",
            bridge_script_pubkey.to_hex_string()
        ))])
        .expect("should be able to get utxos in the bridge address");

    let utxos = utxos.unspents;
    let num_utxos = utxos.len();
    event!(
        Level::DEBUG,
        event = "got utxos in the bridge address",
        num_utxos = num_utxos
    );

    assert_eq!(
        num_utxos, 1,
        "there should be exactly 1 deposit UTXO in the bridge address"
    );

    let bridge_denomination = Amount::from(BRIDGE_DENOMINATION);

    assert_eq!(
        utxos[0].amount, bridge_denomination,
        "the deposit UTXO amount should equal the BRIDGE_DENOMINATION ({}) but got: {}",
        bridge_denomination, utxos[0].amount
    );
}
