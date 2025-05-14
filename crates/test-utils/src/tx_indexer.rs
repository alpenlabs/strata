use bitcoin::{
    block::{Header, Version},
    hashes::Hash,
    Amount, Block, BlockHash, CompactTarget, ScriptBuf, Transaction, TxMerkleNode,
};
use strata_l1tx::{
    filter::indexer::{index_block, TxVisitor},
    messages::Indexed,
    utils::test_utils::create_tx_filter_config,
};
use strata_primitives::l1::{BitcoinAmount, ProtocolOperation};

use crate::{
    bitcoin::{
        build_test_deposit_request_script, build_test_deposit_script, create_test_deposit_tx,
        test_taproot_addr,
    },
    l2::gen_params,
};

// TEST FUNCTIONS

/// Runs a test deposit transaction with the given parameters and returns the indexer's output.
/// The caller can then perform further tests on the output.
pub fn test_index_multiple_deposits_with_visitor<V>(
    visitor: impl Fn() -> V,
    ops_extractor: fn(&Indexed<V::Output>) -> Vec<ProtocolOperation>,
) -> Vec<Indexed<V::Output>>
where
    V: TxVisitor,
{
    let params = gen_params();
    let (filter_config, keypair) = create_tx_filter_config(&params);
    let deposit_config = filter_config.deposit_config.clone();
    let idx1 = 0xdeafbeef;
    let idx2 = 0x1badb007;
    let dest_addr1 = vec![3u8; 20];
    let dest_addr2 = vec![4u8; 20];
    let tapnodehash = [0u8; 32];

    let deposit_script1 =
        build_test_deposit_script(&deposit_config, idx1, dest_addr1.clone(), &tapnodehash);
    let deposit_script2 =
        build_test_deposit_script(&deposit_config, idx2, dest_addr2.clone(), &tapnodehash);

    let tx1 = create_test_deposit_tx(
        Amount::from_sat(deposit_config.deposit_amount),
        &deposit_config.address.address().script_pubkey(),
        &deposit_script1,
        &keypair,
        &tapnodehash,
    );
    let tx2 = create_test_deposit_tx(
        Amount::from_sat(deposit_config.deposit_amount),
        &deposit_config.address.address().script_pubkey(),
        &deposit_script2,
        &keypair,
        &tapnodehash,
    );

    let block = create_test_block(vec![tx1, tx2]);

    let tx_entries = index_block(&block, visitor, &filter_config);

    assert_eq!(
        tx_entries.len(),
        2,
        "test: should find two relevant transactions"
    );

    for (i, info) in tx_entries.iter().flat_map(|e| ops_extractor(e)).enumerate() {
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
                "test: deposit idx should match"
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

    tx_entries
}

pub fn test_index_no_deposit_with_visitor<V>(
    visitor: impl Fn() -> V,
    _: fn(&Indexed<V::Output>) -> Vec<ProtocolOperation>,
) -> Vec<Indexed<V::Output>>
where
    V: TxVisitor,
{
    let params = gen_params();
    let (filter_config, keypair) = create_tx_filter_config(&params);
    let tapnode_hash = [0u8; 32];
    let deposit_config = filter_config.deposit_config.clone();
    let irrelevant_tx = create_test_deposit_tx(
        Amount::from_sat(deposit_config.deposit_amount),
        &test_taproot_addr().address().script_pubkey(),
        &ScriptBuf::new(),
        &keypair,
        &tapnode_hash,
    );

    let block = create_test_block(vec![irrelevant_tx]);

    let tx_entries = index_block(&block, visitor, &filter_config);

    assert!(
        tx_entries.is_empty(),
        "Should not find any relevant transactions"
    );
    tx_entries
}

pub fn test_index_deposit_request_with_visitor<V>(
    visitor_fn: impl Fn() -> V,
    extract_proto_ops: fn(&Indexed<V::Output>) -> Vec<ProtocolOperation>,
) -> Vec<Indexed<V::Output>>
where
    V: TxVisitor,
{
    let params = gen_params();
    let (filter_config, key_pair) = create_tx_filter_config(&params);
    let mut deposit_config = filter_config.deposit_config.clone();

    let extra_amt = 10_000;
    deposit_config.deposit_amount += extra_amt;

    let dest_addr = vec![2u8; 20]; // EVM address
    let dummy_block = [0u8; 32]; // Take-back block hash
    let tapnode_hash = [0u8; 32]; // Taproot tweak

    let deposit_request_script = build_test_deposit_request_script(
        deposit_config.magic_bytes.clone(),
        dummy_block.to_vec(),
        dest_addr.clone(),
    );

    let tx = create_test_deposit_tx(
        Amount::from_sat(deposit_config.deposit_amount),
        &deposit_config.address.address().script_pubkey(),
        &deposit_request_script,
        &key_pair,
        &tapnode_hash,
    );

    let block = create_test_block(vec![tx]);
    let tx_entries = index_block(&block, visitor_fn, &filter_config);

    let deposit_reqs: Vec<_> = tx_entries
        .iter()
        .flat_map(|x| {
            extract_proto_ops(x).into_iter().filter_map(|o| match o {
                ProtocolOperation::DepositRequest(dr) => Some(dr),
                _ => None,
            })
        })
        .collect();

    assert_eq!(deposit_reqs.len(), 1, "Should find one deposit request");

    for dep_req_info in &deposit_reqs {
        assert_eq!(dep_req_info.address, dest_addr, "EE address should match");
        assert_eq!(
            dep_req_info.take_back_leaf_hash, dummy_block,
            "Control block hash should match"
        );
    }
    tx_entries
}

pub fn test_index_deposit_with_visitor<V>(
    visitor_fn: impl Fn() -> V,
    extract_ops: fn(&Indexed<V::Output>) -> Vec<ProtocolOperation>,
) -> Vec<Indexed<V::Output>>
where
    V: TxVisitor,
{
    let params = gen_params();
    let (filter_config, keypair) = create_tx_filter_config(&params);
    let deposit_config = filter_config.deposit_config.clone();

    let idx = 0xdeadbeef;
    let ee_addr = vec![1u8; 20];
    let tapnode_hash = [0u8; 32];

    let deposit_script =
        build_test_deposit_script(&deposit_config, idx, ee_addr.clone(), &tapnode_hash);

    let tx = create_test_deposit_tx(
        Amount::from_sat(deposit_config.deposit_amount),
        &deposit_config.address.address().script_pubkey(),
        &deposit_script,
        &keypair,
        &tapnode_hash,
    );

    let block = create_test_block(vec![tx]);

    let tx_entries = index_block(&block, visitor_fn, &filter_config);

    assert_eq!(tx_entries.len(), 1, "Should find one relevant transaction");

    let ops = extract_ops(&tx_entries[0]);

    assert_eq!(ops.len(), 1, "Should find exactly one protocol operation");

    match &ops[0] {
        ProtocolOperation::Deposit(deposit_info) => {
            assert_eq!(deposit_info.address, ee_addr, "EE address should match");
            assert_eq!(deposit_info.deposit_idx, idx, "Deposit idx should match");
            assert_eq!(
                deposit_info.amt,
                BitcoinAmount::from_sat(deposit_config.deposit_amount),
                "Deposit amount should match"
            );
        }
        _ => panic!("Expected Deposit info"),
    }

    tx_entries
}

// TODO: implement this properly when we need to. For now, trying to support multiple ops is
// creating more issues than helping us.
pub fn test_index_tx_with_multiple_ops_with_visitor<V>(
    visitor: impl Fn() -> V,
    _extract_ops: fn(&Indexed<V::Output>) -> Vec<ProtocolOperation>,
) -> Vec<Indexed<V::Output>>
where
    V: TxVisitor,
{
    // TODO: fill in details as necessary
    let params = gen_params();
    let (filter_config, _) = create_tx_filter_config(&params);

    // Create a block with single tx that has multiple ops
    let block = create_test_block(vec![]);

    let tx_entries = index_block(&block, visitor, &filter_config);
    println!("tx_entries: {:?}", tx_entries.len());

    // TODO: Add tests on tx_entries

    tx_entries
}

// HELPERS

/// Helper function to create a test block with given transactions
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
