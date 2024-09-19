//! Tests the bridge-in flow.
//!
//! This is done by creating the Deposit Request Transaction, manually creating a `DepositInfo` out
//! of it and calling appropriate methods to create the final Deposit Transaction.

use common::bridge::{perform_rollup_actions, perform_user_actions, setup, BridgeDuty, User};
use tracing::{debug, event, info, Level};

mod common;

#[tokio::test]
async fn deposit_flow() {
    let num_operators = 5;

    let (bitcoind, federation) = setup(num_operators).await;

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

    info!("Deposit flow complete");
}
