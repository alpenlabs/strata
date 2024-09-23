use alpen_express_primitives::tx::RelevantTxInfo;
use bitcoin::{Address, Block, Transaction};

use crate::parser::{
    deposit::{
        deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info,
        DepositTxConfig,
    },
    inscription::parse_inscription_data,
};

use super::messages::ProtocolOpTxRef;

/// What kind of transactions can be relevant for rollup node to filter
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelevantTxType {
    /// Transactions that are spent to an address
    SpentToAddress(Address),
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// InscriptionParser which dictates the structure of inscription.
    RollupInscription(RollupName),
    /// Deposit transaction
    Deposit(DepositTxConfig),
}

type RollupName = String;

/// Filter all the relevant [`Transaction`]s in a block based on given [`RelevantTxType`]s
pub fn filter_relevant_txs(
    block: &Block,
    relevent_types: &[RelevantTxType],
) -> Vec<ProtocolOpTxRef> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            check_and_extract_relevant_info(tx, relevent_types)
                .map(|relevant_tx|
                    ProtocolOpTxRef::new(i as u32, relevant_tx)
                    )
        })
        .collect()
}

///  if a [`Transaction`] is relevant based on given [`RelevantTxType`]s then we extract relevant
///  info
fn check_and_extract_relevant_info(
    tx: &Transaction,
    relevant_types: &[RelevantTxType],
) -> Option<RelevantTxInfo> {
    relevant_types.iter().find_map(|rel_type| match rel_type {
        RelevantTxType::SpentToAddress(address) => {
            if tx
                .output
                .iter()
                .any(|op| address.matches_script_pubkey(&op.script_pubkey))
            {
                return Some(RelevantTxInfo::SpentToAddress(
                    address.script_pubkey().to_bytes(),
                ));
            }
            None
        }

        RelevantTxType::RollupInscription(name) => {
            if !tx.input.is_empty() {
                if let Some(scr) = tx.input[0].witness.tapscript() {
                    if let Ok(inscription_data) = parse_inscription_data(&scr.into(), name) {
                        return Some(RelevantTxInfo::RollupInscription(inscription_data));
                    }
                }
            }
            None
        }

        RelevantTxType::Deposit(config) => {
            if let Some(deposit_info) = extract_deposit_info(tx, config) {
                return Some(RelevantTxInfo::Deposit(deposit_info));
            }

            if let Some(deposit_req_info) = extract_deposit_request_info(tx, config) {
                return Some(RelevantTxInfo::DepositRequest(deposit_req_info));
            }

            None
        }
    })
}
#[cfg(test)]
mod test {
    use std::str::FromStr;

    use alpen_express_primitives::tx::InscriptionData;
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

    use super::*;
    use crate::{
        parser::deposit::test_utils::{
            build_test_deposit_request_script, build_test_deposit_script,
            create_transaction_two_outpoints, generic_taproot_addr, get_deposit_tx_config,
        },
        writer::builder::{build_reveal_transaction, generate_inscription_script},
    };

    const OTHER_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";
    const RELEVANT_ADDR: &str = "bcrt1qwqas84jmu20w6r7x3tqetezg8wx7uc3l57vue6";

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

    #[test]
    fn test_filter_relevant_txs_spent_to_address() {
        let address = parse_addr(RELEVANT_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);

        let (txids, _): (Vec<u32>, Vec<RelevantTxInfo>) =
            filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)])
                .into_iter()
                .unzip();
        assert_eq!(txids[0], 0); // Only tx1 matches
    }

    // Create an inscription transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block
    fn create_inscription_tx(rollup_name: String) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let inscription_data = InscriptionData::new(vec![0, 1, 2, 3]);

        let script = generate_inscription_script(inscription_data, &rollup_name, 1).unwrap();

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
        let mut tx = build_reveal_transaction(inp_tx, address, 100, 10, &script, &cb).unwrap();
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

        let (txids, _): (Vec<u32>, Vec<RelevantTxInfo>) =
            filter_relevant_txs(&block, &[RelevantTxType::RollupInscription(rollup_name)])
                .into_iter()
                .unzip();
        assert_eq!(txids[0], 0, "Should filter valid rollup name");

        // Test with invalid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);
        let result = filter_relevant_txs(
            &block,
            &[RelevantTxType::RollupInscription(
                "invalid_name".to_string(),
            )],
        );
        assert!(result.is_empty(), "Should filter out invalid name");
    }

    #[test]
    fn test_filter_relevant_txs_no_match() {
        let address = parse_addr(RELEVANT_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(1000, &parse_addr(OTHER_ADDR))]);
        let tx2 = create_test_tx(vec![create_test_txout(10000, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);

        let (txids, _): (Vec<u32>, Vec<RelevantTxInfo>) =
            filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)])
                .into_iter()
                .unzip();
        assert!(txids.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_relevant_txs_empty_block() {
        let block = create_test_block(vec![]);

        let result = filter_relevant_txs(
            &block,
            &[RelevantTxType::SpentToAddress(parse_addr(RELEVANT_ADDR))],
        );
        assert!(result.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_relevant_txs_multiple_matches() {
        let address = parse_addr(RELEVANT_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let tx3 = create_test_tx(vec![create_test_txout(1000, &address)]);
        let block = create_test_block(vec![tx1, tx2, tx3]);

        let (txids, _): (Vec<u32>, Vec<RelevantTxInfo>) =
            filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)])
                .into_iter()
                .unzip();
        // First and third txs match
        assert_eq!(txids[0], 0);
        assert_eq!(txids[1], 2);
    }

    #[test]
    fn test_filter_relevant_txs_deposit() {
        let config = get_deposit_tx_config();
        let ee_addr = vec![1u8; 20]; // Example EVM address
        let deposit_script =
            build_test_deposit_script(config.magic_bytes.clone(), ee_addr.clone());

        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &config.federation_address.script_pubkey(),
            &deposit_script,
        );

        let block = create_test_block(vec![tx]);

        let relevant_types = vec![RelevantTxType::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &relevant_types);

        assert_eq!(result.len(), 1, "Should find one relevant transaction");
        assert_eq!(
            result[0].0, 0,
            "The relevant transaction should be the first one"
        );

        if let RelevantTxInfo::Deposit(deposit_info) = &result[0].1 {
            assert_eq!(deposit_info.address, ee_addr, "EE address should match");
            assert_eq!(
                deposit_info.amt, config.deposit_quantity,
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
            Amount::from_sat(1000), // Any amount
            &generic_taproot_addr().script_pubkey(),
            &deposit_request_script,
        );

        let block = create_test_block(vec![tx]);

        let relevant_types = vec![RelevantTxType::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &relevant_types);

        assert_eq!(result.len(), 1, "Should find one relevant transaction");
        assert_eq!(
            result[0].0, 0,
            "The relevant transaction should be the first one"
        );

        if let RelevantTxInfo::DepositRequest(deposit_req_info) = &result[0].1 {
            assert_eq!(
                deposit_req_info.address, dest_addr,
                "EE address should match"
            );
            assert_eq!(
                deposit_req_info.tap_ctrl_blk_hash, dummy_block,
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
            Amount::from_sat(1000),
            &generic_taproot_addr().script_pubkey(),
            &ScriptBuf::new(),
        );

        let block = create_test_block(vec![irrelevant_tx]);

        let relevant_types = vec![RelevantTxType::Deposit(config)];
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
            &config.federation_address.script_pubkey(),
            &deposit_script1,
        );
        let tx2 = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &config.federation_address.script_pubkey(),
            &deposit_script2,
        );

        let block = create_test_block(vec![tx1, tx2]);

        let relevant_types = vec![RelevantTxType::Deposit(config.clone())];
        let result = filter_relevant_txs(&block, &relevant_types);

        assert_eq!(result.len(), 2, "Should find two relevant transactions");
        assert_eq!(
            result[0].0, 0,
            "First relevant transaction should be at index 0"
        );
        assert_eq!(
            result[1].0, 1,
            "Second relevant transaction should be at index 1"
        );

        for (i, (_, info)) in result.iter().enumerate() {
            if let RelevantTxInfo::Deposit(deposit_info) = info {
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
                    deposit_info.amt, config.deposit_quantity,
                    "Deposit amount should match for transaction {}",
                    i
                );
            } else {
                panic!("Expected Deposit info for transaction {}", i);
            }
        }
    }
}
