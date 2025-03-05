use bitcoin::{ScriptBuf, Transaction};
use strata_primitives::{
    bitcoin_bosd::Descriptor,
    l1::{BitcoinAmount, WithdrawalFulfillmentInfo},
    sorted_vec::HasKey,
};
use tracing::debug;

use super::TxFilterConfig;

fn create_opreturn_metadata(operator_idx: u32, deposit_idx: u32) -> ScriptBuf {
    let mut metadata = [0u8; 8];
    // first 4 bytes = operator idx
    metadata[..4].copy_from_slice(&operator_idx.to_be_bytes());
    // next 4 bytes = deposit idx
    metadata[4..].copy_from_slice(&deposit_idx.to_be_bytes());
    Descriptor::new_op_return(&metadata).unwrap().to_script()
}

/// Parse transaction and search for a Withdrawal Fulfillment transaction to an expected address.
pub fn parse_withdrawal_fulfillment_transactions<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> Option<WithdrawalFulfillmentInfo> {
    // 1. Check this is of correct structure
    let frontpayment_txout = tx.output.first()?;
    let metadata_txout = tx.output.get(1)?;
    metadata_txout.script_pubkey.is_op_return().then_some(())?;

    // 2. Check withdrawal is to an address we expect
    let withdrawal = filter_conf
        .expected_withdrawal_fulfillments
        .binary_search_by_key(
            &frontpayment_txout.script_pubkey.clone().into(),
            HasKey::get_key,
        )
        .ok()
        .map(|x| filter_conf.expected_withdrawal_fulfillments[x].clone())?;

    // 3. Ensure amount is equal to the expected amount
    let actual_amount_sats = frontpayment_txout.value.to_sat();
    if actual_amount_sats < withdrawal.amount {
        debug!(
            ?withdrawal,
            ?actual_amount_sats,
            "Transaction with expected withdrawal scriptpubkey found, but amount does not match"
        );
        return None;
    }

    // 4. Ensure it has correct metadata of the assigned operator.
    let expected_metadata_script =
        create_opreturn_metadata(withdrawal.operator_idx, withdrawal.deposit_idx);
    if metadata_txout.script_pubkey != expected_metadata_script {
        debug!(
            ?withdrawal,
            ?metadata_txout.script_pubkey,
            "Transaction with expected withdrawal scriptpubkey and amount found, but metadata does not match"
        );
        return None;
    }

    Some(WithdrawalFulfillmentInfo {
        deposit_idx: withdrawal.deposit_idx,
        operator_idx: withdrawal.operator_idx,
        amt: BitcoinAmount::from_sat(actual_amount_sats),
        txid: tx.compute_txid().into(),
    })
}

#[cfg(test)]
mod test {
    use bitcoin::{absolute::LockTime, transaction::Version, Amount, OutPoint, Transaction, TxOut};
    use strata_primitives::{bitcoin_bosd::Descriptor, params::Params};
    use strata_state::bridge_state::{
        DepositEntry, DepositState, DispatchCommand, DispatchedState, WithdrawOutput,
    };
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};

    use super::*;
    use crate::filter::types::{derive_expected_withdrawal_fulfillments, OPERATOR_FEE};

    const DEPOSIT_AMT: Amount = Amount::from_int_btc(10);

    fn deposit_amt() -> BitcoinAmount {
        DEPOSIT_AMT.into()
    }

    fn withdraw_amt_after_fees() -> Amount {
        DEPOSIT_AMT - OPERATOR_FEE
    }

    fn generate_data() -> (Vec<Descriptor>, TxFilterConfig) {
        let params: Params = gen_params();
        let mut gen = ArbitraryGenerator::new();
        let mut addresses = Vec::new();
        for _ in 0..10 {
            addresses.push(Descriptor::new_p2wpkh(&gen.generate()));
        }

        let mut filterconfig = TxFilterConfig::derive_from(params.rollup()).unwrap();

        let create_dispatched_deposit_entry =
            |deposit_idx: u32, assigned_operator_idx: u32, addr: Descriptor, deadline: u64| {
                DepositEntry::new(
                    deposit_idx,
                    OutPoint::null().into(),
                    vec![0, 1, 2],
                    deposit_amt(),
                )
                .with_state(DepositState::Dispatched(DispatchedState::new(
                    DispatchCommand::new(vec![WithdrawOutput::new(
                        addr,
                        Amount::from_btc(10.0).unwrap().into(),
                    )]),
                    assigned_operator_idx,
                    deadline,
                )))
            };

        let deposits = vec![
            // deposits with withdrawal assignments
            create_dispatched_deposit_entry(2, 1, addresses[0].clone(), 100),
            create_dispatched_deposit_entry(3, 2, addresses[1].clone(), 100),
            create_dispatched_deposit_entry(4, 0, addresses[2].clone(), 100),
            // deposits without withdrawal assignments
            DepositEntry::new(5, OutPoint::null().into(), vec![0, 1, 2], deposit_amt())
                .with_state(DepositState::Accepted),
            DepositEntry::new(6, OutPoint::null().into(), vec![0, 1, 2], deposit_amt())
                .with_state(DepositState::Accepted),
        ];

        filterconfig.expected_withdrawal_fulfillments =
            derive_expected_withdrawal_fulfillments(deposits.iter()).into();

        (addresses, filterconfig)
    }

    #[test]
    fn test_parse_withdrawal_fulfillment_transactions_ok() {
        let (addresses, filterconfig) = generate_data();
        let txn = Transaction {
            version: Version(1),
            lock_time: LockTime::from_height(0).unwrap(),
            input: vec![], // dont care
            output: vec![
                // front payment
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
                // metadata with operator index
                TxOut {
                    script_pubkey: create_opreturn_metadata(1, 2),
                    value: Amount::from_sat(0),
                },
                // change
                TxOut {
                    script_pubkey: addresses[4].to_script(),
                    value: Amount::from_btc(0.12345).unwrap(),
                },
            ],
        };

        let withdrawal_fulfillment_info =
            parse_withdrawal_fulfillment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfillment_info.is_some());

        assert_eq!(
            withdrawal_fulfillment_info.unwrap(),
            WithdrawalFulfillmentInfo {
                deposit_idx: 2,
                operator_idx: 1,
                amt: withdraw_amt_after_fees().into(),
                txid: txn.compute_txid().into()
            }
        );
    }

    #[test]
    fn test_parse_withdrawal_fulfillment_transactions_fail_wrong_order() {
        // TESTCASE: valid withdrawal, but different order of txout
        let (addresses, filterconfig) = generate_data();

        let txn = Transaction {
            version: Version(1),
            lock_time: LockTime::from_height(0).unwrap(),
            input: vec![], // dont care
            output: vec![
                // change
                TxOut {
                    script_pubkey: addresses[4].to_script(),
                    value: Amount::from_btc(0.12345).unwrap(),
                },
                // metadata with operator index
                TxOut {
                    script_pubkey: create_opreturn_metadata(1, 2),
                    value: Amount::from_sat(0),
                },
                // front payment
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
            ],
        };

        let withdrawal_fulfillment_info =
            parse_withdrawal_fulfillment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfillment_info.is_none());
    }

    #[test]
    fn test_parse_withdrawal_fulfillment_transactions_fail_wrong_operator() {
        // TESTCASE: correct amount but wrong operator idx for deposit
        let (addresses, filterconfig) = generate_data();

        let txn = Transaction {
            version: Version(1),
            lock_time: LockTime::from_height(0).unwrap(),
            input: vec![], // dont care
            output: vec![
                // front payment
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
                // metadata with operator index
                TxOut {
                    script_pubkey: create_opreturn_metadata(2, 2),
                    value: Amount::from_sat(0),
                },
                // change
                TxOut {
                    script_pubkey: addresses[4].to_script(),
                    value: Amount::from_btc(0.12345).unwrap(),
                },
            ],
        };

        let withdrawal_fulfillment_info =
            parse_withdrawal_fulfillment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfillment_info.is_none());
    }

    #[test]
    fn test_parse_withdrawal_fulfillment_transactions_fail_missing_op_return() {
        let (addresses, filterconfig) = generate_data();

        let txn = Transaction {
            version: Version(1),
            lock_time: LockTime::from_height(0).unwrap(),
            input: vec![],
            output: vec![
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
                // change
                TxOut {
                    script_pubkey: addresses[5].to_script(),
                    value: Amount::from_btc(0.12345).unwrap(),
                },
            ],
        };

        let withdrawal_fulfillment_info =
            parse_withdrawal_fulfillment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfillment_info.is_none())
    }
}
