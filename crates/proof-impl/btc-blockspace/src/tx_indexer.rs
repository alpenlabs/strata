use digest::Digest;
use sha2::Sha256;
use strata_l1tx::filter::indexer::TxIndexer;
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};

/// Ops indexer for Prover. Basically this efficiently gets da commitment from chunks without doing
/// anything else with the chunks.
#[derive(Debug, Clone)]
pub(crate) struct ProverTxIndexer {
    ops: Vec<ProtocolOperation>,
}

impl ProverTxIndexer {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }
}

impl TxIndexer for ProverTxIndexer {
    fn visit_da<'a>(&mut self, chunks: impl Iterator<Item = &'a [u8]>) {
        let mut hasher = Sha256::new();
        for chunk in chunks {
            hasher.update(chunk);
        }
        let hash: [u8; 32] = hasher.finalize().into();
        self.ops.push(ProtocolOperation::DaCommitment(hash.into()));
    }

    fn visit_deposit(&mut self, d: DepositInfo) {
        self.ops.push(ProtocolOperation::Deposit(d));
    }

    fn visit_checkpoint(&mut self, chkpt: SignedBatchCheckpoint) {
        self.ops.push(ProtocolOperation::Checkpoint(chkpt));
    }

    fn collect(
        self,
    ) -> (
        Vec<ProtocolOperation>,
        Vec<strata_state::tx::DepositRequestInfo>,
        Vec<strata_l1tx::messages::DaEntry>,
    ) {
        (self.ops, Vec::new(), Vec::new())
    }
}

/// These are mostly similar to the ones in `strata_btcio::reader::ops_visitor` except for the
/// visitor `ProverOpsVisitor` and indexing of deposit requests.
#[cfg(test)]
mod test {
    use bitcoin::{
        block::{Header, Version},
        hashes::Hash,
        Amount, Block, BlockHash, CompactTarget, ScriptBuf, Transaction, TxMerkleNode,
    };
    use strata_btcio::test_utils::create_checkpoint_envelope_tx;
    use strata_l1tx::filter::{
        indexer::{BlockIndexer, OpIndexer},
        TxFilterConfig,
    };
    use strata_primitives::{
        l1::{payload::L1Payload, BitcoinAmount},
        params::Params,
    };
    use strata_state::{batch::SignedBatchCheckpoint, tx::ProtocolOperation};
    use strata_test_utils::{
        bitcoin::{build_test_deposit_script, create_test_deposit_tx, test_taproot_addr},
        l2::gen_params,
        ArbitraryGenerator,
    };

    use super::ProverTxIndexer;

    const TEST_ADDR: &str = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5";

    //Helper function to create a test block with given transactions
    fn create_test_block(transactions: Vec<Transaction>) -> Block {
        let bhash = BlockHash::from_byte_array([0; 32]);
        Block {
            header: Header {
                version: Version::ONE,
                prev_blockhash: bhash,
                merkle_root: TxMerkleNode::from_byte_array(*bhash.as_byte_array()),
                time: 100,
                bits: CompactTarget::from_consensus(1),
                nonce: 1,
            },
            txdata: transactions,
        }
    }

    fn create_tx_filter_config(params: &Params) -> TxFilterConfig {
        TxFilterConfig::derive_from(params.rollup()).expect("can't get filter config")
    }

    #[test]
    fn test_index_deposits() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
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

        let ops_indexer = OpIndexer::new(ProverTxIndexer::new());
        let (tx_entries, _, _) = ops_indexer.index_block(&block, &filter_config).collect();

        assert_eq!(tx_entries.len(), 1, "Should find one relevant transaction");

        for op in tx_entries[0].proto_ops() {
            if let ProtocolOperation::Deposit(deposit_info) = op {
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
    }

    #[test]
    fn test_index_no_deposit() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
        let deposit_config = filter_config.deposit_config.clone();
        let irrelevant_tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &ScriptBuf::new(),
        );

        let block = create_test_block(vec![irrelevant_tx]);

        let ops_indexer = OpIndexer::new(ProverTxIndexer::new());
        let (tx_entries, _, _) = ops_indexer.index_block(&block, &filter_config).collect();

        assert!(
            tx_entries.is_empty(),
            "Should not find any relevant transactions"
        );
    }

    #[test]
    fn test_index_multiple_deposits() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
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

        let ops_indexer = OpIndexer::new(ProverTxIndexer::new());
        let (tx_entries, _, _) = ops_indexer.index_block(&block, &filter_config).collect();

        assert_eq!(tx_entries.len(), 2, "Should find two relevant transactions");

        for (i, info) in tx_entries
            .iter()
            .flat_map(|op_txs| op_txs.proto_ops())
            .enumerate()
        {
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

    #[test]
    fn test_index_tx_with_multiple_ops() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
        let deposit_config = filter_config.deposit_config.clone();
        let ee_addr = vec![1u8; 20]; // Example EVM address

        // Create deposit utxo
        let deposit_script =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), ee_addr.clone());

        let mut tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script,
        );

        // Create envelope tx and copy its input to the above deposit tx
        let num_envelopes = 1;
        let l1_payloads: Vec<_> = (0..num_envelopes)
            .map(|_| {
                let signed_checkpoint: SignedBatchCheckpoint = ArbitraryGenerator::new().generate();
                L1Payload::new_checkpoint(borsh::to_vec(&signed_checkpoint).unwrap())
            })
            .collect();
        let envtx = create_checkpoint_envelope_tx(&params, TEST_ADDR, l1_payloads);
        tx.input.push(envtx.input[0].clone());

        // Create a block with single tx that has multiple ops
        let block = create_test_block(vec![tx]);

        let ops_indexer = OpIndexer::new(ProverTxIndexer::new());
        let (tx_entries, _, _) = ops_indexer.index_block(&block, &filter_config).collect();

        assert_eq!(
            tx_entries.len(),
            1,
            "Should find one matching transaction entry"
        );
        assert_eq!(
            tx_entries[0].proto_ops().len(),
            2,
            "Should find two protocol ops"
        );

        let mut dep_count = 0;
        let mut ckpt_count = 0;
        for op in tx_entries[0].proto_ops() {
            match op {
                ProtocolOperation::Deposit(_) => dep_count += 1,
                ProtocolOperation::Checkpoint(_) => ckpt_count += 1,
                _ => {}
            }
        }
        assert_eq!(dep_count, 1, "should have one deposit");
        assert_eq!(ckpt_count, 1, "should have one checkpoint");
    }
}
