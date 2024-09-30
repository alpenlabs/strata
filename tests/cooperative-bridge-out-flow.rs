//! Tests the bridge-out flow.
//!
//! This is done by funding the bridge address directly, then manually creating a `WithdrawalInfo`
//! and involving the appropriate functions to create the final withdrawal transaction.

use std::sync::Arc;

use alpen_express_primitives::bridge::{OperatorIdx, PublickeyTable};
use bitcoin::{
    key::rand::{self, Rng},
    Address, Amount, Network, OutPoint, ScriptBuf, Transaction,
};
use bitcoind::BitcoinD;
use common::bridge::{setup, BridgeDuty, User, MIN_MINER_REWARD_CONFS};
use express_bridge_tx_builder::prelude::{
    create_taproot_addr, create_tx, create_tx_ins, create_tx_outs, get_aggregated_pubkey,
    CooperativeWithdrawalInfo, SpendPath, BRIDGE_DENOMINATION,
};
use tokio::sync::Mutex;
use tracing::{event, span, Level};

mod common;

#[tokio::test]
async fn withdrawal_flow() {
    let num_operators = 5;
    let (bitcoind, federation) = setup(num_operators).await;

    let span = span!(Level::WARN, "starting cooperative withdrawal flow");
    let _guard = span.enter();

    event!(
        Level::WARN,
        event = "set up the federation with the bitcoind client",
        num_operators = %num_operators
    );

    let (outpoint, bridge_address) = fund_bridge(federation.pubkey_table, bitcoind.clone()).await;

    event!(Level::INFO, event = "bridge address funded with UTXO", outpoint = %outpoint, bridge_address = %bridge_address);

    let user = User::new("requester", bitcoind.clone()).await;
    let user_x_only_pk = user.agent().pubkey();

    let unspent_utxos_prewithdrawal = user.agent().get_unspent_utxos().await;
    event!(Level::DEBUG, event = "got unspent utxos from requester before withdrawal", num_unspent_utxos = %unspent_utxos_prewithdrawal.len());

    event!(Level::INFO, event = "created user to initiate withdrawal", user_x_only_pk = ?user_x_only_pk);

    let assigned_operator_idx = rand::thread_rng().gen_range(0..num_operators) as OperatorIdx;
    event!(Level::INFO, event = "assigning withdrawal", operator_idx = %assigned_operator_idx);

    let withdrawal_info =
        CooperativeWithdrawalInfo::new(outpoint, user_x_only_pk, assigned_operator_idx, 0);

    event!(Level::DEBUG, action = "creating withdrawal duty", withdrawal_info = ?withdrawal_info);
    let duty = BridgeDuty::Withdrawal(withdrawal_info);

    event!(Level::WARN, action = "dispatching withdrawal duty");

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

    let num_blocks = 1;
    event!(Level::DEBUG, action = "mining some blocks to confirm withdrawal transaction", num_blocks = %num_blocks);
    // the mining reward from this block won't be available for 100 blocks, so does not count
    // towards unspent utxos.
    user.agent().mine_blocks(num_blocks).await;

    let unspent_utxos_postwithdrawal = user.agent().get_unspent_utxos().await;
    event!(Level::DEBUG, event = "got unspent utxos from requester after withdrawal", num_unspent_utxos = %unspent_utxos_postwithdrawal.len());

    assert_eq!(
        unspent_utxos_postwithdrawal.len() - unspent_utxos_prewithdrawal.len(),
        1,
        "user should have one more unspent utxo"
    );

    event!(Level::INFO, event = "Withdrawal flow complete");
}

async fn fund_bridge(
    pubkey_table: PublickeyTable,
    bitcoind: Arc<Mutex<BitcoinD>>,
) -> (OutPoint, Address) {
    let span = span!(Level::INFO, "funding the bridge address");
    let _guard = span.enter();

    event!(
        Level::INFO,
        action = "creating a benefactor to provide initial fund to the bridge"
    );

    let benefactor = User::new("benefactor", bitcoind.clone()).await;

    event!(
        Level::INFO,
        action = "mining blocks to the benefactor's address"
    );
    let balance = benefactor.agent().mine_blocks(MIN_MINER_REWARD_CONFS).await;

    assert!(
        balance.gt(&BRIDGE_DENOMINATION.into()),
        "user balance must be greater than the bridge denomination, got: {}, expected > {}",
        balance,
        BRIDGE_DENOMINATION
    );
    event!(
        Level::INFO,
        action = "getting available utxos for the benefactor"
    );

    let (change_address, outpoint, total_amount) = benefactor
        .agent()
        .select_utxo(BRIDGE_DENOMINATION.into())
        .await
        .expect("should get utxo with enough amount");

    event!(Level::INFO, event = "got change address and outpoint to use", change_address = %change_address, outpoint = %outpoint, amount = %total_amount);

    event!(
        Level::INFO,
        action = "creating transaction to fund the bridge"
    );
    let (unsigned_tx, vout, bridge_addr) =
        create_funding_tx(outpoint, pubkey_table, change_address, total_amount).await;

    event!(
        Level::INFO,
        action = "signing funding transaction with wallet"
    );
    let signed_tx_result = benefactor.agent().sign_raw_tx(&unsigned_tx).await;
    assert!(signed_tx_result.complete, "tx should be fully signed");

    let signed_tx = signed_tx_result
        .transaction()
        .expect("should be able to get fully signed transaction");

    event!(Level::WARN, action = "broadcasting funding tx", signed_tx = ?signed_tx);
    let txid = benefactor.agent().broadcast_signed_tx(&signed_tx).await;
    event!(Level::INFO, event = "broadcasted funding transaction", txid = %txid);

    let num_blocks = 1;
    event!(
        Level::INFO,
        action = "mining some more blocks to confirm the transaction",
        num_blocks = %num_blocks
    );
    benefactor.agent().mine_blocks(num_blocks).await;

    let outpoint = OutPoint { txid, vout };

    (outpoint, bridge_addr)
}

/// Get the transaction that funds the bridge out of a user's UTXO.
///
/// # Returns
///
/// A tuple containing:
///
/// 1. The raw unsigned transaction used to fund the bridge.
/// 2. The output index of the transaction that actually funds the bridge.
/// 3. The bridge address where the funds were sent.
async fn create_funding_tx(
    outpoint: OutPoint,
    pubkey_table: PublickeyTable,
    change_address: Address,
    total_amount: Amount,
) -> (Transaction, u32, Address) {
    let input = create_tx_ins([outpoint]);

    // Outputs in DT:
    // 1) N/N
    // 2) `OP_RETURN` (with zero amount)

    let (bridge_addr, bridge_script_pubkey) = create_bridge_addr(pubkey_table);

    let change_pubkey = change_address.script_pubkey();

    let tx_fees = bridge_script_pubkey.minimal_non_dust() + change_pubkey.minimal_non_dust();
    let change_amount = total_amount - Amount::from(BRIDGE_DENOMINATION) - tx_fees;

    let output = create_tx_outs([
        (bridge_script_pubkey, BRIDGE_DENOMINATION.into()),
        (change_address.script_pubkey(), change_amount),
    ]);

    (create_tx(input, output), 0, bridge_addr)
}

pub(crate) fn create_bridge_addr(pubkey_table: PublickeyTable) -> (Address, ScriptBuf) {
    let aggregated_pubkey = get_aggregated_pubkey(pubkey_table);
    let spend_path = SpendPath::KeySpend {
        internal_key: aggregated_pubkey,
    };

    let (bridge_addr, spend_info) = create_taproot_addr(&Network::Regtest, spend_path)
        .expect("should be able to create bridge address");

    assert!(
        spend_info.merkle_root().is_none(),
        "no merkle root should be present"
    );

    let bridge_script_pubkey = bridge_addr.script_pubkey();

    (bridge_addr, bridge_script_pubkey)
}
