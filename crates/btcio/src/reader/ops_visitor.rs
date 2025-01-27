use digest::Digest;
use sha2::Sha256;
use strata_l1tx::filter::visitor::OpsVisitor;
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};

/// Ops visitor for rollup client.
#[derive(Clone, Debug)]
pub struct ClientOpsVisitor {
    ops: Vec<ProtocolOperation>,
    // TODO: Add l1 manager to store da to db
}

impl ClientOpsVisitor {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }
}

impl OpsVisitor for ClientOpsVisitor {
    fn collect(self) -> Vec<ProtocolOperation> {
        self.ops
    }

    fn visit_da<'a>(&mut self, data: impl Iterator<Item = &'a [u8]>) {
        let mut hasher = Sha256::new();
        for d in data {
            hasher.update(d);
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
}

// #[cfg(test)]
// mod test {
// //Helper function to create a test block with given transactions
//  fn create_test_block(transactions: Vec<Transaction>) -> Block {
//      let bhash = BlockHash::from_byte_array([0; 32]);
//      Block {
//          header: Header {
//              version: BVersion::ONE,
//              prev_blockhash: bhash,
//              merkle_root: TxMerkleNode::from_byte_array(*bhash.as_byte_array()),
//              time: 100,
//              bits: CompactTarget::from_consensus(1),
//              nonce: 1,
//          },
//          txdata: transactions,
//      }
//  }
//     #[test]
//     fn test_filter_relevant_txs_deposit() {
//         let params = gen_params();
//         let filter_config = create_tx_filter_config(&params);
//         let deposit_config = filter_config.deposit_config.clone();
//         let ee_addr = vec![1u8; 20]; // Example EVM address
//         let params = gen_params();
//         let deposit_script =
//             build_test_deposit_script(deposit_config.magic_bytes.clone(), ee_addr.clone());
//
//         let tx = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount),
//             &deposit_config.address.address().script_pubkey(),
//             &deposit_script,
//         );
//
//         let block = create_test_block(vec![tx]);
//
//         let result =
//             filter_protocol_op_tx_refs(&block, params.rollup(), &filter_config, &TestVisitor);
//
//         assert_eq!(result.len(), 1, "Should find one relevant transaction");
//         assert_eq!(
//             result[0].index(),
//             0,
//             "The relevant transaction should be the first one"
//         );
//
//         for op in result[0].proto_ops() {
//             if let ProtocolOperation::Deposit(deposit_info) = op {
//                 assert_eq!(deposit_info.address, ee_addr, "EE address should match");
//                 assert_eq!(
//                     deposit_info.amt,
//                     BitcoinAmount::from_sat(deposit_config.deposit_amount),
//                     "Deposit amount should match"
//                 );
//             } else {
//                 panic!("Expected Deposit info");
//             }
//         }
//     }
//
//     #[test]
//     fn test_filter_multiple_ops_in_single_tx() {
//         let params = gen_params();
//         let filter_config = create_tx_filter_config(&params);
//         let deposit_config = filter_config.deposit_config.clone();
//         let ee_addr = vec![1u8; 20]; // Example EVM address
//
//         // Create deposit utxo
//         let deposit_script =
//             build_test_deposit_script(deposit_config.magic_bytes.clone(), ee_addr.clone());
//
//         let mut tx = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount),
//             &deposit_config.address.address().script_pubkey(),
//             &deposit_script,
//         );
//
//         // Create envelope tx and copy its input to the above deposit tx
//         let num_envelopes = 1;
//         let envtx = create_checkpoint_envelope_tx(&params, num_envelopes);
//         tx.input.push(envtx.input[0].clone());
//
//         // Create a block with single tx that has multiple ops
//         let block = create_test_block(vec![tx]);
//
//         let result =
//             filter_protocol_op_tx_refs(&block, params.rollup(), &filter_config, &TestVisitor);
//
//         assert_eq!(result.len(), 1, "Should find one relevant transaction");
//         assert_eq!(
//             result[0].proto_ops().len(),
//             2,
//             "Should find two protocol ops"
//         );
//
//         let mut dep_count = 0;
//         let mut ckpt_count = 0;
//         for op in result[0].proto_ops() {
//             match op {
//                 ProtocolOperation::Deposit(_) => dep_count += 1,
//                 ProtocolOperation::Checkpoint(_) => ckpt_count += 1,
//                 _ => {}
//             }
//         }
//         assert_eq!(dep_count, 1, "should have one deposit");
//         assert_eq!(ckpt_count, 1, "should have one checkpoint");
//     }
//
//     #[test]
//     fn test_filter_relevant_txs_deposit_request() {
//         let params = gen_params();
//         let filter_config = create_tx_filter_config(&params);
//         let mut deposit_config = filter_config.deposit_config.clone();
//         let params = gen_params();
//         let extra_amt = 10000;
//         deposit_config.deposit_amount += extra_amt;
//         let dest_addr = vec![2u8; 20]; // Example EVM address
//         let dummy_block = [0u8; 32]; // Example dummy block
//         let deposit_request_script = build_test_deposit_request_script(
//             deposit_config.magic_bytes.clone(),
//             dummy_block.to_vec(),
//             dest_addr.clone(),
//         );
//
//         let tx = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount), // Any amount
//             &deposit_config.address.address().script_pubkey(),
//             &deposit_request_script,
//         );
//
//         let block = create_test_block(vec![tx]);
//
//         let result =
//             filter_protocol_op_tx_refs(&block, params.rollup(), &filter_config, &TestVisitor);
//
//         assert_eq!(result.len(), 1, "Should find one relevant transaction");
//         assert_eq!(
//             result[0].index(),
//             0,
//             "The relevant transaction should be the first one"
//         );
//
//         for op in result[0].proto_ops() {
//             if let ProtocolOperation::DepositRequest(deposit_req_info) = op {
//                 assert_eq!(
//                     deposit_req_info.address, dest_addr,
//                     "EE address should match"
//                 );
//                 assert_eq!(
//                     deposit_req_info.take_back_leaf_hash, dummy_block,
//                     "Control block should match"
//                 );
//             } else {
//                 panic!("Expected DepositRequest info");
//             }
//         }
//     }
//
//     #[test]
//     fn test_filter_relevant_txs_no_deposit() {
//         let params = gen_params();
//         let filter_config = create_tx_filter_config(&params);
//         let deposit_config = filter_config.deposit_config.clone();
//         let params = gen_params();
//         let irrelevant_tx = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount),
//             &test_taproot_addr().address().script_pubkey(),
//             &ScriptBuf::new(),
//         );
//
//         let block = create_test_block(vec![irrelevant_tx]);
//
//         let result =
//             filter_protocol_op_tx_refs(&block, params.rollup(), &filter_config, &TestVisitor);
//
//         assert!(
//             result.is_empty(),
//             "Should not find any relevant transactions"
//         );
//     }
//
//     #[test]
//     fn test_filter_relevant_txs_multiple_deposits() {
//         let params = gen_params();
//         let filter_config = create_tx_filter_config(&params);
//         let deposit_config = filter_config.deposit_config.clone();
//         let params = gen_params();
//         let dest_addr1 = vec![3u8; 20];
//         let dest_addr2 = vec![4u8; 20];
//
//         let deposit_script1 =
//             build_test_deposit_script(deposit_config.magic_bytes.clone(), dest_addr1.clone());
//         let deposit_script2 =
//             build_test_deposit_script(deposit_config.magic_bytes.clone(), dest_addr2.clone());
//
//         let tx1 = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount),
//             &deposit_config.address.address().script_pubkey(),
//             &deposit_script1,
//         );
//         let tx2 = create_test_deposit_tx(
//             Amount::from_sat(deposit_config.deposit_amount),
//             &deposit_config.address.address().script_pubkey(),
//             &deposit_script2,
//         );
//
//         let block = create_test_block(vec![tx1, tx2]);
//
//         let result =
//             filter_protocol_op_tx_refs(&block, params.rollup(), &filter_config, &TestVisitor);
//
//         assert_eq!(result.len(), 2, "Should find two relevant transactions");
//         assert_eq!(
//             result[0].index(),
//             0,
//             "First relevant transaction should be at index 0"
//         );
//         assert_eq!(
//             result[1].index(),
//             1,
//             "Second relevant transaction should be at index 1"
//         );
//
//         for (i, info) in result
//             .iter()
//             .flat_map(|op_txs| op_txs.proto_ops())
//             .enumerate()
//         {
//             if let ProtocolOperation::Deposit(deposit_info) = info {
//                 assert_eq!(
//                     deposit_info.address,
//                     if i == 0 {
//                         dest_addr1.clone()
//                     } else {
//                         dest_addr2.clone()
//                     },
//                     "EVM address should match for transaction {}",
//                     i
//                 );
//                 assert_eq!(
//                     deposit_info.amt,
//                     BitcoinAmount::from_sat(deposit_config.deposit_amount),
//                     "Deposit amount should match for transaction {}",
//                     i
//                 );
//             } else {
//                 panic!("Expected Deposit info for transaction {}", i);
//             }
//         }
//     }
// }
