use bitcoin::{Address, Block, Transaction};

/// What kind of transactions can be interesting for us to filter
#[derive(Clone, Debug)]
pub enum TxInterest {
    /// Txs that are spent to an address
    SpentToAddress(Address),
    /// Txs with certain prefix. This can also be used to support matching whole txids
    TxIdWithPrefix(Vec<u8>),
    // Add other interesting conditions as needed
}

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
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use bitcoin::{
        absolute::{Height, LockTime},
        block::{Header, Version as BVersion},
        hashes::Hash,
        transaction::Version,
        Address, Amount, Block, BlockHash, CompactTarget, Network, Transaction, TxMerkleNode,
        TxOut,
    };

    use super::*;

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
