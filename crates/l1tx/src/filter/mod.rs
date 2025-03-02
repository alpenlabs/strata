use bitcoin::Transaction;
use strata_primitives::{
    batch::SignedCheckpoint,
    l1::{
        payload::L1PayloadType, BitcoinAmount, DepositInfo, DepositRequestInfo, DepositSpendInfo,
        WithdrawalFulfilmentInfo,
    },
};
use strata_state::batch::verify_signed_checkpoint_sig;
use tracing::warn;

pub mod indexer;
pub mod types;

pub use types::TxFilterConfig;

use crate::{
    deposit::{deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info},
    envelope::parser::parse_envelope_payloads,
    utils::op_return_nonce,
};

fn parse_deposit_requests(
    tx: &Transaction,
    filter_conf: &TxFilterConfig,
) -> impl Iterator<Item = DepositRequestInfo> {
    // TODO: Currently only one item is parsed, need to check thoroughly and parse multiple
    extract_deposit_request_info(tx, &filter_conf.deposit_config).into_iter()
}

/// Parse deposits from [`Transaction`].
fn parse_deposits(
    tx: &Transaction,
    filter_conf: &TxFilterConfig,
) -> impl Iterator<Item = DepositInfo> {
    // TODO: Currently only one item is parsed, need to check thoroughly and parse multiple
    extract_deposit_info(tx, &filter_conf.deposit_config).into_iter()
}

/// Parse da blobs from [`Transaction`].
fn parse_da_blobs<'a>(
    _tx: &'a Transaction,
    _filter_conf: &TxFilterConfig,
) -> impl Iterator<Item = impl Iterator<Item = &'a [u8]> + 'a> {
    // TODO: actually implement this when we have da
    std::iter::empty::<std::slice::Iter<'a, &'a [u8]>>().map(|inner| inner.copied())
}

/// Parses envelope from the given transaction. Currently, the only envelope recognizable is
/// the checkpoint envelope.
// TODO: we need to change envelope structure and possibly have envelopes for checkpoints and
// DA separately
fn parse_checkpoint_envelopes<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> impl Iterator<Item = SignedCheckpoint> + 'a {
    tx.input.iter().flat_map(move |inp| {
        inp.witness
            .tapscript()
            .and_then(|scr| parse_envelope_payloads(&scr.into(), filter_conf).ok())
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|item| match *item.payload_type() {
                        L1PayloadType::Checkpoint => {
                            borsh::from_slice::<SignedCheckpoint>(item.data())
                                .ok()
                                .filter(|signed_checkpoint| {
                                    verify_signed_checkpoint_sig(
                                        signed_checkpoint,
                                        &filter_conf.sequencer_cred_rule,
                                    )
                                })
                        }
                        L1PayloadType::Da => {
                            warn!("Da parsing is not supported yet");
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
}

/// Parse transaction and search for a Withdrawal Fulfilment transaction to an expected address.
fn parse_withdrawal_fulfilment_transactions<'a>(
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
                Some((actual_amount_sats, info))
            })
    })?;

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

    Some(WithdrawalFulfilmentInfo {
        deposit_idx: info.deposit_idx,
        amt: BitcoinAmount::from_sat(actual_amount_sats),
    })
}

/// Parse transaction and filter out any deposits that have been spent.
fn parse_deposit_spends<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> impl Iterator<Item = DepositSpendInfo> + 'a {
    tx.input.iter().filter_map(|txin| {
        filter_conf
            .expected_outpoints
            .binary_search_by_key(&txin.previous_output, |config| config.output.outpoint())
            .map(|config| DepositSpendInfo {
                deposit_idx: config.deposit_idx,
            })
    })
}

#[cfg(test)]
mod test {
    use bitcoin::{Amount, ScriptBuf};
    use strata_btcio::test_utils::create_checkpoint_envelope_tx;
    use strata_primitives::{
        l1::{payload::L1Payload, BitcoinAmount},
        params::Params,
    };
    use strata_state::batch::SignedCheckpoint;
    use strata_test_utils::{
        bitcoin::{
            build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
            test_taproot_addr,
        },
        l2::gen_params,
        ArbitraryGenerator,
    };

    use super::TxFilterConfig;
    use crate::filter::{parse_checkpoint_envelopes, parse_deposit_requests, parse_deposits};

    const TEST_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";

    /// Helper function to create filter config
    fn create_tx_filter_config(params: &Params) -> TxFilterConfig {
        TxFilterConfig::derive_from(params.rollup()).expect("can't get filter config")
    }

    #[test]
    fn test_parse_envelopes() {
        // Test with valid name
        let mut params: Params = gen_params();
        let filter_config = create_tx_filter_config(&params);

        // Testing multiple envelopes are parsed
        let num_envelopes = 2;
        let l1_payloads: Vec<_> = (0..num_envelopes)
            .map(|_| {
                let signed_checkpoint: SignedCheckpoint = ArbitraryGenerator::new().generate();
                L1Payload::new_checkpoint(borsh::to_vec(&signed_checkpoint).unwrap())
            })
            .collect();
        let tx = create_checkpoint_envelope_tx(&params, TEST_ADDR, l1_payloads.clone());
        let checkpoints: Vec<_> = parse_checkpoint_envelopes(&tx, &filter_config).collect();

        assert_eq!(checkpoints.len(), 2, "Should filter relevant envelopes");

        // Test with invalid checkpoint tag
        params.rollup.checkpoint_tag = "invalid_checkpoint_tag".to_string();

        let tx = create_checkpoint_envelope_tx(&params, TEST_ADDR, l1_payloads);
        let checkpoints: Vec<_> = parse_checkpoint_envelopes(&tx, &filter_config).collect();
        assert!(checkpoints.is_empty(), "There should be no envelopes");
    }

    #[test]
    fn test_parse_deposit_txs() {
        let params = gen_params();
        let filter_conf = create_tx_filter_config(&params);
        let deposit_config = filter_conf.deposit_config.clone();
        let ee_addr = vec![1u8; 20]; // Example EVM address
        let deposit_script =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), ee_addr.clone());

        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script,
        );
        let deposits: Vec<_> = parse_deposits(&tx, &filter_conf).collect();
        assert_eq!(deposits.len(), 1, "Should find one deposit transaction");
        assert_eq!(deposits[0].address, ee_addr, "EE address should match");
        assert_eq!(
            deposits[0].amt,
            BitcoinAmount::from_sat(deposit_config.deposit_amount),
            "Deposit amount should match"
        );
    }

    #[test]
    fn test_parse_deposit_request() {
        let params = gen_params();
        let filter_conf = create_tx_filter_config(&params);
        let mut deposit_conf = filter_conf.deposit_config.clone();

        let extra_amt = 10000;
        deposit_conf.deposit_amount += extra_amt;
        let dest_addr = vec![2u8; 20]; // Example EVM address
        let dummy_block = [0u8; 32]; // Example dummy block
        let deposit_request_script = build_test_deposit_request_script(
            deposit_conf.magic_bytes.clone(),
            dummy_block.to_vec(),
            dest_addr.clone(),
        );

        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_conf.deposit_amount), // Any amount
            &deposit_conf.address.address().script_pubkey(),
            &deposit_request_script,
        );

        let deposit_reqs: Vec<_> = parse_deposit_requests(&tx, &filter_conf).collect();
        assert_eq!(deposit_reqs.len(), 1, "Should find one deposit request");

        assert_eq!(
            deposit_reqs[0].address, dest_addr,
            "EE address should match"
        );
        assert_eq!(
            deposit_reqs[0].take_back_leaf_hash, dummy_block,
            "Control block should match"
        );
    }

    /// Tests parsing deposits which are invalid, i.e won't parse.
    #[test]
    fn test_parse_invalid_deposit() {
        let params = gen_params();
        let filter_conf = create_tx_filter_config(&params);
        let deposit_conf = filter_conf.deposit_config.clone();
        // This won't have magic bytes in script so shouldn't get parsed.
        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_conf.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &ScriptBuf::new(),
        );

        let deposits: Vec<_> = parse_deposits(&tx, &filter_conf).collect();
        assert!(deposits.is_empty(), "Should find no deposit request");
    }
}
