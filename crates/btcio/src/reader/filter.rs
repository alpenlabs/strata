use bitcoin::{Address, Block, Transaction};

use crate::inscription::InscriptionParser;

/// What kind of transactions can be interesting for us to filter
#[derive(Clone, Debug)]
pub enum TxInterest {
    /// Transactions that are spent to an address
    SpentToAddress(Address),
    /// Transactions with certain prefix. This can also be used to support matching whole txids
    TxIdWithPrefix(Vec<u8>),
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// [`InscriptionParser`] which dictates the structure of inscription.
    RollupInscription(RollupName),
    // Add other interesting conditions as needed
}

type RollupName = String;

/// Filter all the interesting [`Transaction`]s in a block based on given interests
pub fn filter_interesting_txs(block: &Block, interests: &[TxInterest]) -> Vec<u32> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter(|(_, tx)| is_interesting(tx, interests))
        .map(|(i, _)| i as u32)
        .collect()
}

/// Determines if a [`Transaction`] is interesting based on given interests
fn is_interesting(tx: &Transaction, interests: &[TxInterest]) -> bool {
    let txid = tx.compute_txid();

    interests.iter().any(|interest| match interest {
        TxInterest::TxIdWithPrefix(pf) => txid[0..pf.len()] == *pf,
        TxInterest::SpentToAddress(address) => tx
            .output
            .iter()
            .any(|op| address.matches_script_pubkey(&op.script_pubkey)),
        TxInterest::RollupInscription(name) => match tx.input[0].witness.tapscript() {
            // Definitely not interesting if it is not a tapscript
            None => false,
            // If it is a tapscript, check rollup name
            Some(scr) => {
                let parser = InscriptionParser::new(scr.into());
                parser
                    .parse_rollup_name()
                    .ok()
                    .filter(|n| n == name)
                    .is_some()
            }
        },
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

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
    use crate::{inscription::InscriptionData, writer::builder::build_reveal_transaction};

    const OTHER_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";
    const INTERESTING_ADDR: &str = "bcrt1qwqas84jmu20w6r7x3tqetezg8wx7uc3l57vue6";

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
    fn test_filter_interesting_txs_spent_to_address() {
        let address = parse_addr(INTERESTING_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);

        let result = filter_interesting_txs(&block, &[TxInterest::SpentToAddress(address)]);
        assert_eq!(result, vec![0]); // Only tx1 matches
    }

    #[test]
    fn test_filter_interesting_txs_txid_with_prefix() {
        let address = parse_addr(INTERESTING_ADDR);
        let address1 = parse_addr(OTHER_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &address1)]);
        let block = create_test_block(vec![tx1, tx2.clone()]);
        let txid = {
            let a = tx2.compute_txid();
            *a.as_byte_array()
        };

        let result =
            filter_interesting_txs(&block, &[TxInterest::TxIdWithPrefix(txid[0..5].to_vec())]);
        assert_eq!(result, vec![1]); // Only tx2 matches
    }

    // Create an inscription transaction. The focus here is to create a tapscript, rather than a
    // completely valid control block
    fn create_inscription_tx(rollup_name: String) -> Transaction {
        let address = parse_addr(OTHER_ADDR);
        let inp_tx = create_test_tx(vec![create_test_txout(100000000, &address)]);
        let inscription_data = InscriptionData::new(rollup_name.clone(), vec![0, 1, 2, 3]);
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
    fn test_filter_interesting_txs_with_rollup_inscription() {
        // Test with valid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);
        let result = filter_interesting_txs(&block, &[TxInterest::RollupInscription(rollup_name)]);
        assert_eq!(result, vec![0], "Should filter valid rollup name");

        // Test with invalid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);
        let result = filter_interesting_txs(
            &block,
            &[TxInterest::RollupInscription("invalid_name".to_string())],
        );
        assert!(result.is_empty(), "Should filter out invalid name");
    }

    #[test]
    fn test_filter_interesting_txs_no_match() {
        let address = parse_addr(INTERESTING_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(1000, &parse_addr(OTHER_ADDR))]);
        let tx2 = create_test_tx(vec![create_test_txout(10000, &parse_addr(OTHER_ADDR))]);
        let block = create_test_block(vec![tx1, tx2]);

        let result = filter_interesting_txs(&block, &[TxInterest::SpentToAddress(address)]);
        assert!(result.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_interesting_txs_empty_block() {
        let block = create_test_block(vec![]);

        let result = filter_interesting_txs(
            &block,
            &[TxInterest::SpentToAddress(parse_addr(INTERESTING_ADDR))],
        );
        assert!(result.is_empty()); // No transactions match
    }

    #[test]
    fn test_filter_interesting_txs_multiple_matches() {
        let address = parse_addr(INTERESTING_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let tx3 = create_test_tx(vec![create_test_txout(1000, &address)]);
        let block = create_test_block(vec![tx1, tx2, tx3]);

        let result = filter_interesting_txs(&block, &[TxInterest::SpentToAddress(address)]);
        assert_eq!(result, vec![0, 2]); // First and third txs match
    }

    #[test]
    fn test_filter_all_txs() {
        let address = parse_addr(INTERESTING_ADDR);
        let tx1 = create_test_tx(vec![create_test_txout(100, &address)]);
        let tx2 = create_test_tx(vec![create_test_txout(100, &parse_addr(OTHER_ADDR))]);
        let tx3 = create_test_tx(vec![create_test_txout(1000, &address)]);
        let block = create_test_block(vec![tx1, tx2, tx3]);

        let result = filter_interesting_txs(&block, &[TxInterest::TxIdWithPrefix(Vec::new())]);
        assert_eq!(result, vec![0, 1, 2]); // All txs match
    }
}
