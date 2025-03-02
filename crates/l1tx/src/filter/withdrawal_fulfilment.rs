use bitcoin::Transaction;
use strata_primitives::l1::{BitcoinAmount, WithdrawalFulfilmentInfo};

use super::TxFilterConfig;
use crate::utils::op_return_nonce;

/// Parse transaction and search for a Withdrawal Fulfilment transaction to an expected address.
pub fn parse_withdrawal_fulfilment_transactions<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> Option<WithdrawalFulfilmentInfo> {
    // 1. Check this is a txn to a watched address
    let (actual_amount_sats, info) = tx.output.iter().find_map(|txout| {
        filter_conf
            .expected_withdrawal_fulfilments
            .binary_search_by_key(&txout.script_pubkey, |expected| {
                expected.destination.inner()
            })
            .and_then(|info| {
                // 2. Ensure amount is greater than or equal to the expected amount
                let actual_amount_sats = txout.value.to_sat();
                if actual_amount_sats < info.amount {
                    return None;
                }

                // 3. Ensure it has correct metadata of the assigned operator.
                let mut metadata = [0u8; 8];
                // first 4 bytes = operator idx
                metadata[..4].copy_from_slice(&info.operator_idx.to_be_bytes());
                // next 4 bytes = deposit idx
                metadata[4..].copy_from_slice(&info.deposit_idx.to_be_bytes());
                let op_return_script = op_return_nonce(&metadata[..]);
                tx.output
                    .iter()
                    .find(|tx| tx.script_pubkey == op_return_script)?;

                Some((actual_amount_sats, info))
            })
    })?;

    Some(WithdrawalFulfilmentInfo {
        deposit_idx: info.deposit_idx,
        amt: BitcoinAmount::from_sat(actual_amount_sats),
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
    use crate::filter::types::{derive_expected_withdrawal_fulfilments, OPERATOR_FEE};

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

        filterconfig.expected_withdrawal_fulfilments =
            derive_expected_withdrawal_fulfilments(deposits.iter());

        (addresses, filterconfig)
    }

    #[test]
    fn test_parse_withdrawal_fulfilment_transactions_ok() {
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
                    /* {operatoridx: 1u32, depositidx: 2u32} */
                    script_pubkey: op_return_nonce(&[0, 0, 0, 1, 0, 0, 0, 2]),
                    value: Amount::from_sat(0),
                },
                // change
                TxOut {
                    script_pubkey: addresses[4].to_script(),
                    value: Amount::from_btc(0.12345).unwrap(),
                },
            ],
        };

        let withdrawal_fulfilment_info =
            parse_withdrawal_fulfilment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfilment_info.is_some());

        assert_eq!(
            withdrawal_fulfilment_info.unwrap(),
            WithdrawalFulfilmentInfo {
                deposit_idx: 2,
                amt: withdraw_amt_after_fees().into()
            }
        );
    }

    #[test]
    fn test_parse_withdrawal_fulfilment_transactions_ok_random_order() {
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
                // another change
                TxOut {
                    script_pubkey: addresses[5].to_script(),
                    value: Amount::from_btc(1.12345).unwrap(),
                },
                // metadata with operator index
                TxOut {
                    /* {operatoridx: 1u32, depositidx: 2u32} */
                    script_pubkey: op_return_nonce(&[0, 0, 0, 1, 0, 0, 0, 2]),
                    value: Amount::from_sat(0),
                },
                // another change
                TxOut {
                    script_pubkey: addresses[6].to_script(),
                    value: Amount::from_btc(1.12345).unwrap(),
                },
                // front payment
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
            ],
        };

        let withdrawal_fulfilment_info =
            parse_withdrawal_fulfilment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfilment_info.is_some());

        assert_eq!(
            withdrawal_fulfilment_info.unwrap(),
            WithdrawalFulfilmentInfo {
                deposit_idx: 2,
                amt: withdraw_amt_after_fees().into()
            }
        );
    }

    #[test]
    fn test_parse_withdrawal_fulfilment_transactions_ok_double_withdrawal_output() {
        // TESTCASE: valid withdrawal, but there another utxo to a valid withdrawal address
        let (addresses, filterconfig) = generate_data();

        let txn = Transaction {
            version: Version(1),
            lock_time: LockTime::from_height(0).unwrap(),
            input: vec![], // dont care
            output: vec![
                // another txout with a withdrawal address
                TxOut {
                    script_pubkey: addresses[1].to_script(),
                    value: withdraw_amt_after_fees(),
                },
                // metadata with operator index
                TxOut {
                    /* {operatoridx: 1u32, depositidx: 2u32} */
                    script_pubkey: op_return_nonce(&[0, 0, 0, 1, 0, 0, 0, 2]),
                    value: Amount::from_sat(0),
                },
                // correct front payment
                TxOut {
                    script_pubkey: addresses[0].to_script(),
                    value: withdraw_amt_after_fees(),
                },
            ],
        };

        let withdrawal_fulfilment_info =
            parse_withdrawal_fulfilment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfilment_info.is_some());

        assert_eq!(
            withdrawal_fulfilment_info.unwrap(),
            WithdrawalFulfilmentInfo {
                deposit_idx: 2,
                amt: withdraw_amt_after_fees().into()
            }
        );
    }

    #[test]
    fn test_parse_withdrawal_fulfilment_transactions_missing_op_return() {
        let (addresses, filterconfig) = generate_data();

        // TESTCASE: missing op return metadata
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

        let withdrawal_fulfilment_info =
            parse_withdrawal_fulfilment_transactions(&txn, &filterconfig);
        assert!(withdrawal_fulfilment_info.is_none());
    }
}
