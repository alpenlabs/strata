use alpen_express_state::{batch::SignedBatchCheckpoint, tx::ProtocolOperation};
use bitcoin::{Block, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use super::messages::ProtocolOpTxRef;
use crate::{
    deposit::{
        deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info,
        DepositTxConfig,
    },
    inscription::parse_inscription_data,
};

/// kind of transactions can be relevant for rollup node to filter
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum TxFilterRule {
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// InscriptionParser which dictates the structure of inscription.
    RollupInscription(RollupName),
    /// Deposit transaction
    Deposit(DepositTxConfig),
}

type RollupName = String;

/// Filter all the relevant [`Transaction`]s in a block based on given [`TxFilterRule`]s
pub fn filter_relevant_txs(block: &Block, filters: &[TxFilterRule]) -> Vec<ProtocolOpTxRef> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            check_and_extract_relevant_info(tx, filters)
                .map(|relevant_tx| ProtocolOpTxRef::new(i as u32, relevant_tx))
        })
        .collect()
}

///  if a [`Transaction`] is relevant based on given [`RelevantTxType`]s then we extract relevant
///  info
fn check_and_extract_relevant_info(
    tx: &Transaction,
    filters: &[TxFilterRule],
) -> Option<ProtocolOperation> {
    filters.iter().find_map(|rel_type| match rel_type {
        TxFilterRule::RollupInscription(name) => {
            if !tx.input.is_empty() {
                if let Some(scr) = tx.input[0].witness.tapscript() {
                    if let Ok(inscription_data) = parse_inscription_data(&scr.into(), name) {
                        if let Ok(signed_batch) = borsh::from_slice::<SignedBatchCheckpoint>(
                            inscription_data.batch_data(),
                        ) {
                            return Some(ProtocolOperation::RollupInscription(signed_batch));
                        }
                    }
                }
            }
            None
        }

        TxFilterRule::Deposit(config) => {
            if let Some(deposit_info) = extract_deposit_info(tx, config) {
                return Some(ProtocolOperation::Deposit(deposit_info));
            }

            if let Some(deposit_req_info) = extract_deposit_request_info(tx, config) {
                return Some(ProtocolOperation::DepositRequest(deposit_req_info));
            }

            None
        }
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use alpen_express_btcio::test_utils::{
        build_reveal_transaction_test, generate_inscription_script_test,
    };
    use alpen_express_primitives::l1::BitcoinAmount;
    use alpen_express_state::{
        batch::SignedBatchCheckpoint,
        tx::{InscriptionData, ProtocolOperation},
    };
    use alpen_test_utils::ArbitraryGenerator;
    use bitcoin::{
        absolute::{Height, LockTime},
        block::{Header, Version as BVersion},
        hashes::Hash,
        key::{Parity, Secp256k1, UntweakedKeypair},
        secp256k1::XOnlyPublicKey,
        taproot::{ControlBlock, LeafVersion, TaprootMerkleBranch},
        transaction::Version,
        Address, Amount, Block, BlockHash, CompactTarget, Network, ScriptBuf, TapNodeHash,
        Transaction, TxMerkleNode, TxOut,
    };
    use rand::RngCore;

    use crate::{
        deposit::test_utils::{
            build_test_deposit_request_script, build_test_deposit_script,
            create_transaction_two_outpoints, generic_taproot_addr, get_deposit_tx_config,
        },
        filter::{filter_relevant_txs, TxFilterRule},
    };

    const OTHER_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";

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

    // Create an inscription transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block
    fn create_inscription_tx(rollup_name: String) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let signed_checkpoint: SignedBatchCheckpoint = ArbitraryGenerator::new().generate();
        let inscription_data = InscriptionData::new(borsh::to_vec(&signed_checkpoint).unwrap());

        let script = generate_inscription_script_test(inscription_data, &rollup_name, 1).unwrap();

        // Create controlblock
        let secp256k1 = Secp256k1::new();
        let mut rand_bytes = [0; 32];
        rand::thread_rng().fill_bytes(&mut rand_bytes);
        let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &rand_bytes).unwrap();
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
    fn test_filter_relevant_txs_with_rollup_inscription() {
        // Test with valid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);

        let txids: Vec<u32> =
            filter_relevant_txs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
                .iter()
                .map(|op_refs| op_refs.index())
                .collect();

        assert_eq!(txids[0], 0, "Should filter valid rollup name");

        // Test with invalid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);
        let result = filter_relevant_txs(
            &block,
            &[TxFilterRule::RollupInscription("invalid_name".to_string())],
        );
        assert!(result.is_empty(), "Should filter out invalid name");
    }

    #[test]
    fn test_filter_relevant_txs_no_match() {
        let tx1 = create_test_tx(vec![create_test_txout(1000, &parse_addr(OTHER_ADDR))]);
        let tx2 = create_test_tx(vec![create_test_txout(10000, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);
        let rollup_name = "alpenstrata".to_string();

        let txids: Vec<u32> =
            filter_relevant_txs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
                .iter()
                .map(|op_refs| op_refs.index())
                .collect();
        assert!(txids.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_relevant_txs_multiple_matches() {
        let rollup_name = "alpenstrata".to_string();
        let tx1 = create_inscription_tx(rollup_name.clone());
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let tx3 = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx1, tx2, tx3]);

        let txids: Vec<u32> =
            filter_relevant_txs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
                .iter()
                .map(|op_refs| op_refs.index())
                .collect();
        // First and third txs match
        assert_eq!(txids[0], 0);
        assert_eq!(txids[1], 2);
    }

    #[test]
    fn test_filter_relevant_txs_deposit() {
        let config = get_deposit_tx_config();
        let ee_addr = vec![1u8; 20]; // Example EVM address
        let deposit_script = build_test_deposit_script(config.magic_bytes.clone(), ee_addr.clone());

        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr().script_pubkey(),
            &deposit_script,
        );

        let block = create_test_block(vec![tx]);

        let filters = vec![TxFilterRule::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &filters);

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
                BitcoinAmount::from_sat(config.deposit_quantity),
                "Deposit amount should match"
            );
        } else {
            panic!("Expected Deposit info");
        }
    }

    #[test]
    fn test_filter_relevant_txs_deposit_request() {
        let config = get_deposit_tx_config();
        let dest_addr = vec![2u8; 20]; // Example EVM address
        let dummy_block = [0u8; 32]; // Example dummy block
        let deposit_request_script = build_test_deposit_request_script(
            config.magic_bytes.clone(),
            dummy_block.to_vec(),
            dest_addr.clone(),
        );

        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity), // Any amount
            &generic_taproot_addr().script_pubkey(),
            &deposit_request_script,
        );

        let block = create_test_block(vec![tx]);

        let relevant_types = vec![TxFilterRule::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &relevant_types);

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
        let config = get_deposit_tx_config();
        let irrelevant_tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr().script_pubkey(),
            &ScriptBuf::new(),
        );

        let block = create_test_block(vec![irrelevant_tx]);

        let relevant_types = vec![TxFilterRule::Deposit(config)];
        let result = filter_relevant_txs(&block, &relevant_types);

        assert!(
            result.is_empty(),
            "Should not find any relevant transactions"
        );
    }

    #[test]
    fn test_filter_relevant_txs_multiple_deposits() {
        let config = get_deposit_tx_config();
        let dest_addr1 = vec![3u8; 20];
        let dest_addr2 = vec![4u8; 20];

        let deposit_script1 =
            build_test_deposit_script(config.magic_bytes.clone(), dest_addr1.clone());
        let deposit_script2 =
            build_test_deposit_script(config.magic_bytes.clone(), dest_addr2.clone());

        let tx1 = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr().script_pubkey(),
            &deposit_script1,
        );
        let tx2 = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr().script_pubkey(),
            &deposit_script2,
        );

        let block = create_test_block(vec![tx1, tx2]);

        let relevant_types = vec![TxFilterRule::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &relevant_types);

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
                    BitcoinAmount::from_sat(config.deposit_quantity),
                    "Deposit amount should match for transaction {}",
                    i
                );
            } else {
                panic!("Expected Deposit info for transaction {}", i);
            }
        }
    }
}
