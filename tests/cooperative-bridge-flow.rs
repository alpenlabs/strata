//! Tests the full bridge-in and bridge-out flow.

use std::sync::Arc;

use bitcoincore_rpc::{
    bitcoin::{
        key::rand::{rngs::OsRng, Rng},
        Network, OutPoint,
    },
    json::ScanTxOutRequest,
    Client, RpcApi,
};
use common::bridge::{perform_rollup_actions, perform_user_actions, setup, BridgeDuty, User};
use strata_bridge_tx_builder::prelude::{
    create_taproot_addr, get_aggregated_pubkey, CooperativeWithdrawalInfo, SpendPath,
};
use strata_primitives::bridge::{OperatorIdx, PublickeyTable};
use tracing::{debug, event, span, Level};

mod common;

#[tokio::test]
async fn full_flow() {
    let num_operators = 3;

    let (bitcoind, client, bridge_in_federation) = setup(num_operators).await;
    let bridge_out_federation = bridge_in_federation.duplicate("bridge-out").await;

    let deposit_guard = span!(Level::WARN, "Initiating Deposit").entered();

    let user = User::new("end-user", bitcoind.clone()).await;
    event!(Level::INFO, event = "User created", address = ?user.address());

    let (txid, take_back_leaf_hash, taproot_addr, el_address) =
        perform_user_actions(&user, bridge_in_federation.pubkey_table.clone()).await;

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
    for mut operator in bridge_in_federation.operators {
        let duty = duty.clone();
        handles.push(tokio::spawn(async move {
            operator.process_duty(duty).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    event!(
        Level::DEBUG,
        action = "Mining a few blocks to confirm deposit"
    );
    let blocks_to_confirm_deposit = 1;
    // mining this block has the side-effect of confirming one of the blocks pre-mined during
    // `perform_user_actions`.
    user.agent().mine_blocks(blocks_to_confirm_deposit).await;

    let outpoint =
        get_bridge_out_outpoint(bridge_in_federation.pubkey_table.clone(), client.clone()).await;

    event!(Level::INFO, event = "Deposit flow complete");

    deposit_guard.exit();

    let span = span!(Level::WARN, "Initiating Withdrawal");
    let _guard = span.enter();

    let user_x_only_pk = user.agent().pubkey();

    let unspent_utxos_prewithdrawal = user.agent().get_unspent_utxos().await;
    event!(Level::DEBUG, event = "got unspent utxos from requester before withdrawal", num_unspent_utxos = %unspent_utxos_prewithdrawal.len());

    let assigned_operator_idx = OsRng.gen_range(0..num_operators) as OperatorIdx;
    event!(Level::INFO, event = "assigning withdrawal", operator_idx = %assigned_operator_idx);

    let withdrawal_info =
        CooperativeWithdrawalInfo::new(outpoint, user_x_only_pk, assigned_operator_idx, 0);

    event!(Level::DEBUG, action = "creating withdrawal duty", withdrawal_info = ?withdrawal_info);
    let duty = BridgeDuty::Withdrawal(withdrawal_info);

    event!(Level::WARN, action = "dispatching withdrawal duty");

    let mut handles = Vec::new();
    for mut operator in bridge_out_federation.operators {
        let duty = duty.clone();
        handles.push(tokio::spawn(async move {
            operator.process_duty(duty).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let blocks_to_confirm_withdrawal = 1;
    event!(Level::DEBUG, action = "mining some blocks to confirm withdrawal transaction", num_blocks = %blocks_to_confirm_withdrawal);
    // mining this block has the side-effect of confirming `blocks_to_confirm_withdrawal` number of
    // the blocks pre-mined during `perform_user_actions`.
    user.agent().mine_blocks(blocks_to_confirm_withdrawal).await;

    let unspent_utxos_postwithdrawal = user.agent().get_unspent_utxos().await;
    event!(Level::DEBUG, event = "got unspent utxos from requester after withdrawal", num_unspent_utxos = %unspent_utxos_postwithdrawal.len());

    assert_eq!(
        unspent_utxos_postwithdrawal.len() - unspent_utxos_prewithdrawal.len(),
        (blocks_to_confirm_deposit + blocks_to_confirm_withdrawal) as usize,
        "user should have more unspent utxos -- those mined and one withdrawn"
    );

    event!(Level::INFO, event = "Withdrawal flow complete");
}

async fn get_bridge_out_outpoint(pubkey_table: PublickeyTable, client: Arc<Client>) -> OutPoint {
    let aggregated_pubkey = get_aggregated_pubkey(pubkey_table);
    let spend_path = SpendPath::KeySpend {
        internal_key: aggregated_pubkey,
    };
    let (bridge_addr, _) = create_taproot_addr(&Network::Regtest, spend_path)
        .expect("should be able to create the address");

    let bridge_script_pubkey = bridge_addr.script_pubkey();

    let result = client.scan_tx_out_set_blocking(&[ScanTxOutRequest::Single(format!(
        "raw({})",
        bridge_script_pubkey.to_hex_string()
    ))]);

    assert!(
        result.is_ok(),
        "should be able to perform scan txout but got error: {:?}",
        result.unwrap_err()
    );

    let result = result.expect("should be ok");

    let unspent = result
        .unspents
        .first()
        .expect("bridge address should have a deposit utxo");

    OutPoint {
        txid: unspent.txid,
        vout: unspent.vout,
    }
}
