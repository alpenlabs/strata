use bitcoin::{ScriptBuf, Transaction};
use strata_primitives::{
    buf::Buf32,
    l1::{BitcoinAmount, WithdrawalFulfillmentInfo},
};
use tracing::debug;

use super::TxFilterConfig;

/// Parse transaction and search for a Withdrawal Fulfillment transaction to an expected address.
pub fn parse_withdrawal_fulfillment_transactions<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> Option<WithdrawalFulfillmentInfo> {
    // 1. Check this is of correct structure
    let frontpayment_txout = tx.output.first()?;
    let metadata_txout = tx.output.get(1)?;
    let txid: Buf32 = tx.compute_txid().into();

    metadata_txout.script_pubkey.is_op_return().then_some(())?;

    // 2. Ensure correct OP_RETURN data and check it has expected deposit index.
    let (op_idx, dep_idx, deposit_txid_bytes) =
        parse_opreturn_metadata(&metadata_txout.script_pubkey)?;

    let exp_ful = filter_conf.expected_withdrawal_fulfillments.get(&dep_idx)?;

    if exp_ful.operator_idx != op_idx {
        debug!(?txid, "Deposit index matches but operator_idx does not");
        return None;
    }

    // 3. Ensure deposit txid in metadata is correct
    if exp_ful.deposit_txid != deposit_txid_bytes {
        debug!(
            ?txid,
            "Deposit index and operator index matches but deposit txid does not"
        );
        return None;
    }

    // 4. Check if it is spent to expected destination.
    if frontpayment_txout.script_pubkey != *exp_ful.destination.inner() {
        debug!(
            ?txid,
            "Deposit index and operator index matches but script_pubkey does not"
        );
        return None;
    }

    // 5. Ensure amount is equal to the expected amount
    let actual_amount_sats = frontpayment_txout.value.to_sat();
    if actual_amount_sats < exp_ful.amount {
        debug!(
            ?txid,
            "Deposit index and script_pubkey match but the amount does not"
        );
        return None;
    }

    Some(WithdrawalFulfillmentInfo {
        deposit_idx: exp_ful.deposit_idx,
        operator_idx: exp_ful.operator_idx,
        amt: BitcoinAmount::from_sat(actual_amount_sats),
        txid: tx.compute_txid().into(),
    })
}

fn parse_opreturn_metadata(script_buf: &ScriptBuf) -> Option<(u32, u32, [u8; 32])> {
    let opreturn_data = match script_buf.as_bytes() {
        [_, _, data @ ..] => data,
        _ => return None,
    };

    // 4 bytes op idx + 4 bytes dep idx + 32 bytes txid
    if opreturn_data.len() != 40 {
        return None;
    }
    let mut idx_bytes = [0u8; 4];

    idx_bytes.copy_from_slice(&opreturn_data[0..4]);
    let opidx: u32 = u32::from_be_bytes(idx_bytes);

    idx_bytes.copy_from_slice(&opreturn_data[4..8]);
    let depidx: u32 = u32::from_be_bytes(idx_bytes);

    let deposit_txid_bytes = opreturn_data[8..].try_into().unwrap();

    Some((opidx, depidx, deposit_txid_bytes))
}

#[cfg(test)]
mod test {
    use bitcoin::{
        absolute::LockTime, consensus, transaction::Version, Amount, OutPoint, Transaction, TxOut,
    };
    use strata_primitives::{bitcoin_bosd::Descriptor, l1::OutputRef, params::Params};
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

    fn create_opreturn_metadata(
        operator_idx: u32,
        deposit_idx: u32,
        deposit_txid: &[u8; 32],
    ) -> ScriptBuf {
        let mut metadata = [0u8; 40];
        // first 4 bytes = operator idx
        metadata[..4].copy_from_slice(&operator_idx.to_be_bytes());
        // next 4 bytes = deposit idx
        metadata[4..8].copy_from_slice(&deposit_idx.to_be_bytes());
        metadata[8..40].copy_from_slice(deposit_txid);
        Descriptor::new_op_return(&metadata).unwrap().to_script()
    }

    fn create_outputref(txid_bytes: &[u8; 32], vout: u32) -> OutputRef {
        OutPoint::new(consensus::deserialize(txid_bytes).unwrap(), vout).into()
    }

    fn generate_data() -> (Vec<Descriptor>, Vec<[u8; 32]>, TxFilterConfig) {
        let params: Params = gen_params();
        let mut gen = ArbitraryGenerator::new();
        let mut addresses = Vec::new();
        let mut txids = Vec::<[u8; 32]>::new();
        for _ in 0..10 {
            addresses.push(Descriptor::new_p2wpkh(&gen.generate()));
            txids.push(gen.generate());
        }

        let mut filterconfig = TxFilterConfig::derive_from(params.rollup()).unwrap();

        let create_dispatched_deposit_entry =
            |operator_idx: u32,
             deposit_idx: u32,
             addr: Descriptor,
             deadline: u64,
             deposit_txid: &[u8; 32]| {
                DepositEntry::new(
                    deposit_idx,
                    create_outputref(deposit_txid, 0),
                    vec![0, 1, 2],
                    deposit_amt(),
                )
                .with_state(DepositState::Dispatched(DispatchedState::new(
                    DispatchCommand::new(vec![WithdrawOutput::new(
                        addr,
                        Amount::from_btc(10.0).unwrap().into(),
                    )]),
                    operator_idx,
                    deadline,
                )))
            };

        let deposits = vec![
            // deposits with withdrawal assignments
            create_dispatched_deposit_entry(1, 2, addresses[0].clone(), 100, &txids[0]),
            create_dispatched_deposit_entry(2, 3, addresses[1].clone(), 100, &txids[1]),
            create_dispatched_deposit_entry(0, 4, addresses[2].clone(), 100, &txids[2]),
            // deposits without withdrawal assignments
            DepositEntry::new(
                5,
                create_outputref(&txids[3], 0),
                vec![0, 1, 2],
                deposit_amt(),
            )
            .with_state(DepositState::Accepted),
            DepositEntry::new(
                6,
                create_outputref(&txids[4], 0),
                vec![0, 1, 2],
                deposit_amt(),
            )
            .with_state(DepositState::Accepted),
        ];

        filterconfig.expected_withdrawal_fulfillments =
            derive_expected_withdrawal_fulfillments(deposits.iter());

        (addresses, txids, filterconfig)
    }

    #[test]
    fn test_parse_withdrawal_fulfillment_transactions_ok() {
        let (addresses, txids, filterconfig) = generate_data();
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
                    script_pubkey: create_opreturn_metadata(1, 2, &txids[0]),
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
        let (addresses, txids, filterconfig) = generate_data();

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
                    script_pubkey: create_opreturn_metadata(1, 2, &txids[0]),
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
        let (addresses, txids, filterconfig) = generate_data();

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
                    script_pubkey: create_opreturn_metadata(2, 2, &txids[0]),
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
    fn test_parse_withdrawal_fulfillment_transactions_fail_wrong_deposit_txid() {
        // TESTCASE: correct amount and operator idx for deposit, but wrong deposit txid
        let (addresses, txids, filterconfig) = generate_data();

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
                    script_pubkey: create_opreturn_metadata(1, 2, &txids[5]),
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
        let (addresses, _txids, filterconfig) = generate_data();

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
