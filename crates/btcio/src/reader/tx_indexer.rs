use strata_l1tx::{
    filter::indexer::TxVisitor,
    messages::{DaEntry, L1TxMessages},
};
use strata_primitives::{
    batch::SignedCheckpoint,
    l1::{
        DepositInfo, DepositRequestInfo, DepositSpendInfo, ProtocolOperation,
        WithdrawalFulfillmentInfo,
    },
};

/// Ops indexer for rollup client. Collects extra info like da blobs and deposit requests
#[derive(Clone, Debug)]
pub struct ReaderTxVisitorImpl {
    ops: Vec<ProtocolOperation>,
    deposit_requests: Vec<DepositRequestInfo>,
    da_entries: Vec<DaEntry>,
}

impl ReaderTxVisitorImpl {
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            deposit_requests: Vec::new(),
            da_entries: Vec::new(),
        }
    }

    fn ops(&self) -> &[ProtocolOperation] {
        &self.ops
    }
}

impl TxVisitor for ReaderTxVisitorImpl {
    type Output = L1TxMessages;

    fn visit_da<'a>(&mut self, chunks: impl Iterator<Item = &'a [u8]>) {
        let da_entry = DaEntry::from_chunks(chunks);
        self.ops
            .push(ProtocolOperation::DaCommitment(*da_entry.commitment()));
        self.da_entries.push(da_entry);
    }

    fn visit_deposit(&mut self, d: DepositInfo) {
        self.ops.push(ProtocolOperation::Deposit(d));
    }

    fn visit_deposit_request(&mut self, dr: DepositRequestInfo) {
        self.ops.push(ProtocolOperation::DepositRequest(dr.clone()));
        self.deposit_requests.push(dr);
    }

    fn visit_checkpoint(&mut self, chkpt: SignedCheckpoint) {
        self.ops.push(ProtocolOperation::Checkpoint(chkpt));
    }

    fn visit_withdrawal_fulfillment(&mut self, info: WithdrawalFulfillmentInfo) {
        self.ops
            .push(ProtocolOperation::WithdrawalFulfillment(info));
    }

    fn visit_deposit_spend(&mut self, info: DepositSpendInfo) {
        self.ops.push(ProtocolOperation::DepositSpent(info));
    }

    fn finalize(self) -> Option<L1TxMessages> {
        if self.ops.is_empty() && self.deposit_requests.is_empty() && self.da_entries.is_empty() {
            None
        } else {
            Some(L1TxMessages::new(
                self.ops,
                self.deposit_requests,
                self.da_entries,
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use bitcoin::{
        block::{Header, Version},
        hashes::Hash,
        Amount, Block, BlockHash, CompactTarget, ScriptBuf, Transaction, TxMerkleNode,
    };
    use strata_l1tx::filter::{indexer::index_block, TxFilterConfig};
    use strata_primitives::{
        l1::{payload::L1Payload, BitcoinAmount, ProtocolOperation},
        params::Params,
    };
    use strata_test_utils::{
        bitcoin::{
            build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
            test_taproot_addr,
        },
        l2::{gen_params, get_test_signed_checkpoint},
    };

    use crate::{
        reader::tx_indexer::ReaderTxVisitorImpl, test_utils::create_checkpoint_envelope_tx,
    };

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
        let idx = 0xdeadbeef;
        let ee_addr = vec![1u8; 20]; // Example EVM address
        let deposit_script =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), idx, ee_addr.clone());

        let tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script,
        );

        let block = create_test_block(vec![tx]);

        let tx_entries = index_block(&block, ReaderTxVisitorImpl::new, &filter_config);

        assert_eq!(tx_entries.len(), 1, "Should find one relevant transaction");

        for op in tx_entries[0].contents().protocol_ops() {
            if let ProtocolOperation::Deposit(deposit_info) = op {
                assert_eq!(deposit_info.address, ee_addr, "test: dest should match");
                assert_eq!(deposit_info.deposit_idx, idx, "test: idx should match");
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
    fn test_index_txs_deposit_request() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
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

        let tx_entries = index_block(&block, ReaderTxVisitorImpl::new, &filter_config);
        let dep_reqs = tx_entries
            .iter()
            .flat_map(|tx| tx.contents().deposit_reqs())
            .collect::<Vec<_>>();

        assert_eq!(dep_reqs.len(), 1, "Should find one deposit request");

        for dep_req_info in dep_reqs {
            assert_eq!(dep_req_info.address, dest_addr, "EE address should match");
            assert_eq!(
                dep_req_info.take_back_leaf_hash, dummy_block,
                "Control block should match"
            );
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

        let tx_entries = index_block(&block, ReaderTxVisitorImpl::new, &filter_config);

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
        let idx1 = 0xdeadbeef;
        let idx2 = 0x1badb007;
        let dest_addr1 = vec![3u8; 20];
        let dest_addr2 = vec![4u8; 20];

        let deposit_script1 =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), idx1, dest_addr1.clone());
        let deposit_script2 =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), idx2, dest_addr2.clone());

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

        let tx_entries = index_block(&block, ReaderTxVisitorImpl::new, &filter_config);

        assert_eq!(tx_entries.len(), 2, "Should find two relevant transactions");

        for (i, info) in tx_entries
            .iter()
            .flat_map(|op_txs| op_txs.contents().protocol_ops())
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
                    "test: dest should match for transaction {i}",
                );
                assert_eq!(
                    deposit_info.deposit_idx,
                    [idx1, idx2][i],
                    "test: idx should match"
                );
                assert_eq!(
                    deposit_info.amt,
                    BitcoinAmount::from_sat(deposit_config.deposit_amount),
                    "test: deposit amount should match for transaction {i}",
                );
            } else {
                panic!("test: expected DepositInfo for transaction {i}");
            }
        }
    }

    #[test]
    fn test_index_tx_with_multiple_ops() {
        let params = gen_params();
        let filter_config = create_tx_filter_config(&params);
        let deposit_config = filter_config.deposit_config.clone();
        let idx = 0xdeadbeef;
        let ee_addr = vec![1u8; 20]; // Example EVM address

        // Create deposit utxo
        let deposit_script =
            build_test_deposit_script(deposit_config.magic_bytes.clone(), idx, ee_addr.clone());

        let mut tx = create_test_deposit_tx(
            Amount::from_sat(deposit_config.deposit_amount),
            &deposit_config.address.address().script_pubkey(),
            &deposit_script,
        );

        // Create envelope tx and copy its input to the above deposit tx
        let num_envelopes = 1;
        let l1_payloads: Vec<_> = (0..num_envelopes)
            .map(|_| {
                let signed_checkpoint = get_test_signed_checkpoint();
                L1Payload::new_checkpoint(borsh::to_vec(&signed_checkpoint).unwrap())
            })
            .collect();
        let envtx = create_checkpoint_envelope_tx(&params, TEST_ADDR, l1_payloads);
        tx.input.push(envtx.input[0].clone());

        // Create a block with single tx that has multiple ops
        let block = create_test_block(vec![tx]);

        let tx_entries = index_block(&block, ReaderTxVisitorImpl::new, &filter_config);
        println!("tx_entries: {:?}", tx_entries);

        assert_eq!(
            tx_entries.len(),
            1,
            "Should find one matching transaction entry"
        );
        assert_eq!(
            tx_entries[0].contents().protocol_ops().len(),
            2,
            "Should find two protocol ops"
        );

        let mut dep_count = 0;
        let mut ckpt_count = 0;
        for op in tx_entries[0].contents().protocol_ops() {
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
