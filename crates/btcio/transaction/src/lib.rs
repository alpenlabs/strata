//! Structure for filtering relevant transactions

pub mod deposit;
pub mod messages;
pub mod reveal;
pub mod types;
pub mod utils;
pub mod visitor;

use bitcoin::{Block, Transaction};
use strata_state::tx::ProtocolOperation;
use visitor::{BridgeVisitor, OpTxVisitor, ScanProofOpVisitor, TxVisitor};

use crate::messages::ProtocolOpTxRef;
pub use crate::types::TxFilterConfig;

/// Filter protocol operations as refs from relevant [`Transaction`]s in a block based on given
/// [`TxFilterConfig`]s
pub fn filter_protocol_op_tx_refs(
    block: &Block,
    filter_config: TxFilterConfig,
) -> Vec<ProtocolOpTxRef> {
    block
        .txdata
        .iter()
        .enumerate()
        .flat_map(|(i, tx)| {
            extract_protocol_ops(tx, &filter_config)
                .into_iter()
                .map(move |relevant_tx| ProtocolOpTxRef::new(i as u32, relevant_tx))
        })
        .collect()
}

/// Extracts protocol operations from a transaction based on the provided filter configuration.
pub fn extract_protocol_ops(
    tx: &Transaction,
    filter_conf: &TxFilterConfig,
) -> Vec<ProtocolOperation> {
    let scan_proof_op_visitor = ScanProofOpVisitor::new();
    let mut proof_tx_visitor = OpTxVisitor::new(scan_proof_op_visitor);

    // TODO: We shouldn't be including bridge visitor here. As we would want different
    // functions(path) for proof side and client side.
    // we need to concretize what the requirements of client and rework ProtocolOperation
    // (i.e : whole DA chunk to be stored for reconstruction(short and long version),
    // stuff like not having DepositRequest Tx in testnet etc) which
    // needs lifting in separate ticket STR-402
    let mut bridge_visitor = BridgeVisitor::new();

    proof_tx_visitor.visit_tx(filter_conf, tx);
    bridge_visitor.visit_tx(filter_conf, tx);

    proof_tx_visitor
        .op_visitor()
        .protocol_ops()
        .iter()
        .cloned()
        .chain(
            bridge_visitor
                .deposit_requests()
                .iter()
                .cloned()
                .map(ProtocolOperation::DepositRequest),
        )
        .collect()
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use bitcoin::{
        absolute::{Height, LockTime},
        block::{Header, Version as BVersion},
        hashes::Hash,
        key::{Parity, UntweakedKeypair},
        secp256k1::{XOnlyPublicKey, SECP256K1},
        taproot::{ControlBlock, LeafVersion, TaprootMerkleBranch},
        transaction::Version,
        Address, Amount, Block, BlockHash, CompactTarget, Network, ScriptBuf, TapNodeHash,
        Transaction, TxMerkleNode, TxOut,
    };
    use rand::{rngs::OsRng, RngCore};
    use strata_primitives::l1::BitcoinAmount;
    use strata_state::tx::{EnvelopePayload, PayloadTypeTag, ProtocolOperation};
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};

    use super::TxFilterConfig;
    use crate::{
        deposit::test_utils::{
            build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
            test_taproot_addr,
        },
        filter_protocol_op_tx_refs,
        messages::ProtocolOpTxRef,
        reveal::builder::{build_reveal_transaction, generate_envelope_script},
    };

    const OTHER_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";

    /// Helper function to create filter config
    fn create_tx_filter_config() -> TxFilterConfig {
        let params = gen_params();
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

    /// Helper function to create a test block with given transactions
    fn create_test_block(transactions: Vec<Transaction>) -> Block {
        let bhash = BlockHash::from_byte_array([0; 32]);
        Block {
            header: Header {
                version: BVersion::ONE,
                prev_blockhash: bhash,
                merkle_root: TxMerkleNode::from_byte_array(*bhash.as_byte_array()),
                time: 100,
                bits: CompactTarget::from_consensus(1),
                nonce: 1,
            },
            txdata: transactions,
        }
    }

    fn parse_addr(addr: &str) -> Address {
        Address::from_str(addr)
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap()
    }

    // Create an commit reveal transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block
    fn create_commit_reveal_tx(da_tag: &str, ckpt_tag: &str, num_envelopes: u32) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let signed_checkpoint: Vec<u8> = ArbitraryGenerator::new().generate();
        let envelope_data = (0..num_envelopes)
            .map(|_| EnvelopePayload::new(PayloadTypeTag::DA, signed_checkpoint.clone()))
            .collect::<Vec<_>>();

        let script = generate_envelope_script(&envelope_data, da_tag, ckpt_tag).unwrap();

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
        let mut tx = build_reveal_transaction(inp_tx, address, 100, 10, &script, &cb).unwrap();
        tx.input[0].witness.push([1; 3]);
        tx.input[0].witness.push(script);
        tx.input[0].witness.push(cb.serialize());
        tx
    }

    #[test]
    fn test_filter_relevant_txs_with_commit_reveal() {
        // Test with valid tags
        let filter_config = create_tx_filter_config();

        let da_tag = filter_config.da_tag.clone();
        let ckpt_tag = filter_config.ckpt_tag.clone();
        let tx = create_commit_reveal_tx(&da_tag, &ckpt_tag, 1);
        let block = create_test_block(vec![tx]);

        let txids: Vec<u32> = filter_protocol_op_tx_refs(&block, filter_config.clone())
            .iter()
            .map(|op_refs| op_refs.index())
            .collect();

        assert_eq!(txids[0], 0, "Should filter valid tags");

        // Test with invalid tag
        let tx = create_commit_reveal_tx("invalid-da-tag", &ckpt_tag, 1);
        let block = create_test_block(vec![tx]);
        let result = filter_protocol_op_tx_refs(&block, filter_config);
        assert!(result.is_empty(), "Should filter out invalid tag");
    }

    #[test]
    fn test_filter_relevant_txs_of_commit_reveal_tx_with_multiple_envelope() {
        let filter_config = create_tx_filter_config();
        let num_envelopes = 20;
        let da_tag = filter_config.da_tag.clone();
        let ckpt_tag = filter_config.ckpt_tag.clone();
        let tx = create_commit_reveal_tx(&da_tag, &ckpt_tag, num_envelopes);
        let block = create_test_block(vec![tx]);

        let txids: Vec<ProtocolOpTxRef> = filter_protocol_op_tx_refs(&block, filter_config);

        assert_eq!(txids[0].index(), 0, "Should filter valid tags");
        assert_eq!(
            txids.len(),
            num_envelopes as usize,
            "Should have protocolOps equal to number of envelopes"
        );
    }

    #[test]
    fn test_filter_relevant_txs_no_match() {
        let tx1 = create_test_tx(vec![create_test_txout(1000, &parse_addr(OTHER_ADDR))]);
        let tx2 = create_test_tx(vec![create_test_txout(10000, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);
        let filter_config = create_tx_filter_config();

        let txids: Vec<u32> = filter_protocol_op_tx_refs(&block, filter_config)
            .iter()
            .map(|op_refs| op_refs.index())
            .collect();
        assert!(txids.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_relevant_txs_multiple_matches() {
        let filter_config = create_tx_filter_config();
        let da_tag = filter_config.da_tag.clone();
        let ckpt_tag = filter_config.ckpt_tag.clone();
        let tx1 = create_commit_reveal_tx(&da_tag, &ckpt_tag, 1);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let tx3 = create_commit_reveal_tx(&da_tag, &ckpt_tag, 1);
        let block = create_test_block(vec![tx1, tx2, tx3]);

        let txids: Vec<u32> = filter_protocol_op_tx_refs(&block, filter_config)
            .iter()
            .map(|op_refs| op_refs.index())
            .collect();
        // First and third txs match
        assert_eq!(txids[0], 0);
        assert_eq!(txids[1], 2);
    }

    #[test]
    fn test_filter_relevant_txs_deposit() {
        let filter_config = create_tx_filter_config();
        let deposit_config = filter_config.deposit_config.clone();
        let ee_addr = vec![1u8; 20]; // Example EVM address
        let deposit_script =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), ee_addr.clone());

        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script,
        );

        let block = create_test_block(vec![tx]);

        let result = filter_protocol_op_tx_refs(&block, filter_config);

        assert_eq!(result.len(), 1, "Should find one relevant transaction");
        assert_eq!(
            result[0].index(),
            0,
            "The relevant transaction should be the first one"
        );

        if let ProtocolOperation::Deposit(deposit_info) = &result[0].proto_op() {
            assert_eq!(deposit_info.address, ee_addr, "EE address should match");
            assert_eq!(
                deposit_info.amt,
                BitcoinAmount::from_sat(deposit_config.deposit_amount),
                "Deposit amount should match"
            );
        } else {
            panic!("Expected Deposit info");
        }
    }

    #[test]
    fn test_filter_relevant_txs_deposit_request() {
        let filter_config = create_tx_filter_config();
        let mut deposit_config = filter_config.deposit_config.clone();
        let extra_amt = 10000;
        deposit_config.deposit_amount += extra_amt;
        let dest_addr = vec![2u8; 20]; // Example EVM address
        let dummy_block = [0u8; 32]; // Example dummy block
        let deposit_request_script = build_test_deposit_request_script(
            deposit_config.magic_bytes.clone(),
            dummy_block.to_vec(),
            dest_addr.clone(),
        );

        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount), // Any amount
            &deposit_config.address.address().script_pubkey(),
            &deposit_request_script,
        );

        let block = create_test_block(vec![tx]);

        let result = filter_protocol_op_tx_refs(&block, filter_config);

        assert_eq!(result.len(), 1, "Should find one relevant transaction");
        assert_eq!(
            result[0].index(),
            0,
            "The relevant transaction should be the first one"
        );

        if let ProtocolOperation::DepositRequest(deposit_req_info) = &result[0].proto_op() {
            assert_eq!(
                deposit_req_info.address, dest_addr,
                "EE address should match"
            );
            assert_eq!(
                deposit_req_info.take_back_leaf_hash, dummy_block,
                "Control block should match"
            );
        } else {
            panic!("Expected DepositRequest info");
        }
    }

    #[test]
    fn test_filter_relevant_txs_no_deposit() {
        let filter_config = create_tx_filter_config();
        let deposit_config = filter_config.deposit_config.clone();
        let irrelevant_tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &ScriptBuf::new(),
        );

        let block = create_test_block(vec![irrelevant_tx]);

        let result = filter_protocol_op_tx_refs(&block, filter_config);

        assert!(
            result.is_empty(),
            "Should not find any relevant transactions"
        );
    }

    #[test]
    fn test_filter_relevant_txs_multiple_deposits() {
        let filter_config = create_tx_filter_config();
        let deposit_config = filter_config.deposit_config.clone();
        let dest_addr1 = vec![3u8; 20];
        let dest_addr2 = vec![4u8; 20];

        let deposit_script1 =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), dest_addr1.clone());
        let deposit_script2 =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), dest_addr2.clone());

        let tx1 = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script1,
        );
        let tx2 = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script2,
        );

        let block = create_test_block(vec![tx1, tx2]);

        let result = filter_protocol_op_tx_refs(&block, filter_config);

        assert_eq!(result.len(), 2, "Should find two relevant transactions");
        assert_eq!(
            result[0].index(),
            0,
            "First relevant transaction should be at index 0"
        );
        assert_eq!(
            result[1].index(),
            1,
            "Second relevant transaction should be at index 1"
        );

        for (i, info) in result.iter().map(|op_txs| op_txs.proto_op()).enumerate() {
            if let ProtocolOperation::Deposit(deposit_info) = info {
                assert_eq!(
                    deposit_info.address,
                    if i == 0 {
                        dest_addr1.clone()
                    } else {
                        dest_addr2.clone()
                    },
                    "EVM address should match for transaction {}",
                    i
                );
                assert_eq!(
                    deposit_info.amt,
                    BitcoinAmount::from_sat(deposit_config.deposit_amount),
                    "Deposit amount should match for transaction {}",
                    i
                );
            } else {
                panic!("Expected Deposit info for transaction {}", i);
            }
        }
    }
}
