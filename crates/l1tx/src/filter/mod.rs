use bitcoin::Transaction;
use strata_primitives::l1::payload::L1PayloadType;
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, DepositRequestInfo},
};
use tracing::warn;

pub mod types;
pub mod visitor;

pub use types::TxFilterConfig;

use crate::{
    deposit::{deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info},
    envelope::parser::parse_envelope_payloads,
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
fn parse_da<'a>(
    _tx: &'a Transaction,
    _filter_conf: &TxFilterConfig,
) -> impl Iterator<Item = &'a [u8]> {
    // TODO: implement this when we have da
    std::iter::empty()
}

/// Parses envelope from the given transaction. Currently, the only envelope recognizable is
/// the checkpoint envelope.
// TODO: we need to change envelope structure and possibly have envelopes for checkpoints and
// DA separately
fn parse_checkpoint_envelopes<'a>(
    tx: &'a Transaction,
    filter_conf: &'a TxFilterConfig,
) -> impl Iterator<Item = SignedBatchCheckpoint> + 'a {
    tx.input.iter().flat_map(|inp| {
        inp.witness
            .tapscript()
            .and_then(|scr| parse_envelope_payloads(&scr.into(), filter_conf).ok())
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|item| match *item.payload_type() {
                        L1PayloadType::Checkpoint => {
                            borsh::from_slice::<SignedBatchCheckpoint>(item.data()).ok()
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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use bitcoin::{
        absolute::{Height, LockTime},
        key::{Parity, UntweakedKeypair},
        secp256k1::{XOnlyPublicKey, SECP256K1},
        taproot::{ControlBlock, LeafVersion, TaprootMerkleBranch},
        transaction::Version,
        Address, Amount, Network, ScriptBuf, TapNodeHash, Transaction, TxOut,
    };
    use rand::{rngs::OsRng, RngCore};
    use strata_btcio::test_utils::{build_reveal_transaction_test, generate_envelope_script_test};
    use strata_primitives::{
        l1::{payload::L1Payload, BitcoinAmount},
        params::Params,
    };
    use strata_state::batch::SignedBatchCheckpoint;
    use strata_test_utils::{
        bitcoin::{
            build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
        },
        l2::gen_params,
        ArbitraryGenerator,
    };

    use super::TxFilterConfig;
    use crate::{
        deposit::test_utils::test_taproot_addr,
        filter::{parse_checkpoint_envelopes, parse_deposit_requests, parse_deposits},
    };

    const OTHER_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";

    /// Helper function to create filter config
    fn create_tx_filter_config(params: &Params) -> TxFilterConfig {
        TxFilterConfig::derive_from(params.rollup()).expect("can't get filter config")
    }

    /// Helper function to create a test transaction with given txid and outputs
    fn create_test_tx(outputs: Vec<TxOut>) -> Transaction {
        Transaction {
            version: Version(1),
            lock_time: LockTime::Blocks(Height::from_consensus(1).unwrap()),
            input: vec![],
            output: outputs,
        }
    }

    /// Helper function to create a TxOut with a given address and value
    fn create_test_txout(value: u64, address: &Address) -> TxOut {
        TxOut {
            value: Amount::from_sat(value),
            script_pubkey: address.script_pubkey(),
        }
    }

    fn parse_addr(addr: &str) -> Address {
        Address::from_str(addr)
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap()
    }

    // Create an envelope transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block. Includes `n_envelopes` envelopes in the tapscript.
    fn create_checkpoint_envelope_tx(params: &Params, n_envelopes: u32) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let payloads: Vec<_> = (0..n_envelopes)
            .map(|_| {
                let signed_checkpoint: SignedBatchCheckpoint = ArbitraryGenerator::new().generate();
                L1Payload::new_checkpoint(borsh::to_vec(&signed_checkpoint).unwrap())
            })
            .collect();
        let script = generate_envelope_script_test(&payloads, params).unwrap();
        // Create controlblock
        let mut rand_bytes = [0; 32];
        OsRng.fill_bytes(&mut rand_bytes);
        let key_pair = UntweakedKeypair::from_seckey_slice(SECP256K1, &rand_bytes).unwrap();
        let public_key = XOnlyPublicKey::from_keypair(&key_pair).0;
        let nodehash: [TapNodeHash; 0] = [];
        let cb = ControlBlock {
            leaf_version: LeafVersion::TapScript,
            output_key_parity: Parity::Even,
            internal_key: public_key,
            merkle_branch: TaprootMerkleBranch::from(nodehash),
        };

        // Create transaction using control block
        let mut tx = build_reveal_transaction_test(inp_tx, address, 100, 10, &script, &cb).unwrap();
        tx.input[0].witness.push([1; 3]);
        tx.input[0].witness.push(script);
        tx.input[0].witness.push(cb.serialize());
        tx
    }

    #[test]
    fn test_parse_envelopes() {
        // Test with valid name
        let mut params: Params = gen_params();
        let filter_config = create_tx_filter_config(&params);

        // Testing multiple envelopes are parsed
        let num_envelopes = 2;
        let tx = create_checkpoint_envelope_tx(&params, num_envelopes);
        let checkpoints: Vec<_> = parse_checkpoint_envelopes(&tx, &filter_config).collect();

        assert_eq!(checkpoints.len(), 2, "Should filter relevant envelopes");

        // Test with invalid checkpoint tag
        params.rollup.checkpoint_tag = "invalid_checkpoint_tag".to_string();
        let filter_config = create_tx_filter_config(&params);

        let tx = create_checkpoint_envelope_tx(&params, 2);
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
