use bitcoin::Transaction;
use strata_primitives::l1::{DepositInfo, DepositRequestInfo, DepositSpendInfo};

mod checkpoint;
pub mod indexer;
pub mod types;
mod withdrawal_fulfillment;

use checkpoint::parse_valid_checkpoint_envelopes;
pub use types::TxFilterConfig;
use withdrawal_fulfillment::parse_withdrawal_fulfillment_transactions;

use crate::deposit::{
    deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info,
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

use strata_primitives::l1::OutputRef;

/// Parse transaction and filter out any deposits that have been spent.
fn parse_deposit_spends<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> impl Iterator<Item = DepositSpendInfo> + 'a {
    tx.input.iter().filter_map(|txin| {
        let prevout = OutputRef::new(txin.previous_output.txid, txin.previous_output.vout);
        filter_conf
            .expected_outpoints
            .get(&prevout)
            .map(|config| DepositSpendInfo {
                deposit_idx: config.deposit_idx,
            })
    })
}

#[cfg(test)]
mod test {
    use bitcoin::{Amount, ScriptBuf};
    use strata_primitives::{l1::BitcoinAmount, params::Params};
    use strata_test_utils::{
        bitcoin::{
            build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
            test_taproot_addr,
        },
        l2::gen_params,
    };

    use super::TxFilterConfig;
    use crate::filter::{parse_deposit_requests, parse_deposits};

    /// Helper function to create filter config
    fn create_tx_filter_config(params: &Params) -> TxFilterConfig {
        TxFilterConfig::derive_from(params.rollup()).expect("can't get filter config")
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
