use alpen_express_primitives::tx::ParsedTx;
use bitcoin::{Address, Block, Transaction};

use crate::parser::{
    deposit::{
        deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info,
        DepositTxConfig,
    },
    inscription::parse_inscription_data,
};

/// What kind of transactions can be relevant for rollup node to filter
#[derive(Clone, Debug)]
pub enum RelevantTxType {
    /// Transactions that are spent to an address
    SpentToAddress(Address),
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// InscriptionParser which dictates the structure of inscription.
    RollupInscription(RollupName),
    /// Deposit transaction
    Deposit(RollupName, FederationAddress, AddressLength, Amount),
}

type RollupName = String;
type AddressLength = u8;
type Amount = u64;
type FederationAddress = Address;

/// Filter all the relevant [`Transaction`]s in a block based on given [`RelevantTxType`]s
pub fn filter_relevant_txs(
    block: &Block,
    relevent_types: &[RelevantTxType],
) -> Vec<(u32, ParsedTx)> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            check_and_extract_relevancy(tx, relevent_types)
                .map(|relevant_tx| (i as u32, relevant_tx))
        })
        .collect()
}

///  if a [`Transaction`] is relevant based on given [`RelevantTxType`]s then we extract relevant
///  info
fn check_and_extract_relevancy(
    tx: &Transaction,
    relevant_types: &[RelevantTxType],
) -> Option<ParsedTx> {
    for rel_type in relevant_types {
        match rel_type {
            RelevantTxType::SpentToAddress(address) => {
                if tx
                    .output
                    .iter()
                    .any(|op| address.matches_script_pubkey(&op.script_pubkey))
                {
                    return Some(ParsedTx::SpentToAddress(address.script_pubkey().to_bytes()));
                }
            }

            RelevantTxType::RollupInscription(name) => {
                if let Some(scr) = tx.input[0].witness.tapscript() {
                    if let Ok(inscription_data) = parse_inscription_data(&scr.into(), name) {
                        return Some(ParsedTx::RollupInscription(inscription_data));
                    }
                }
            }

            RelevantTxType::Deposit(rollup_name, federation_address, addr_len, amount) => {
                let config = &DepositTxConfig {
                    magic_bytes: rollup_name.to_string().as_bytes().to_vec(),
                    address_length: *addr_len,
                    deposit_quantity: *amount,
                    federation_address: federation_address.clone(),
                };

                if let Ok(deposit_info) = extract_deposit_info(tx, config) {
                    return Some(ParsedTx::Deposit(deposit_info));
                }

                if let Some(deposit_req_info) = extract_deposit_request_info(tx, config) {
                    return Some(ParsedTx::DepositRequest(deposit_req_info));
                }
            }
        }
    }

    None
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
        Address, Amount, Block, BlockHash, CompactTarget, Network, TapNodeHash, Transaction,
        TxMerkleNode, TxOut,
    };
    use rand::RngCore;

    use super::*;
    use crate::writer::builder::build_reveal_transaction;

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

        let result = filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)]);
        assert_eq!(result[0].0, 0); // Only tx1 matches
    }

    // Create an inscription transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block
    fn create_inscription_tx(rollup_name: String) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let inscription_data = InscriptionData::new(rollup_name, vec![0, 1, 2, 3], 1);
        let script = inscription_data.to_script().unwrap();

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
        let result = filter_relevant_txs(&block, &[RelevantTxType::RollupInscription(rollup_name)]);
        println!("{:?}", result);
        assert_eq!(result[0].0, 0, "Should filter valid rollup name");

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

        let result = filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)]);
        assert!(result.is_empty()); // No transactions match
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

        let result = filter_relevant_txs(&block, &[RelevantTxType::SpentToAddress(address)]);
        // First and third txs match
        assert_eq!(result[0].0, 0);
        assert_eq!(result[1].0, 2);
    }
}
