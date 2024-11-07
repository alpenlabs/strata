use bitcoin::{Block, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    buf::Buf32,
    l1::BitcoinAddress,
    params::{OperatorConfig, RollupParams},
    prelude::DepositTxParams,
};
use strata_state::{batch::SignedBatchCheckpoint, tx::ProtocolOperation};

use super::messages::ProtocolOpTxRef;
use crate::{
    deposit::{deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info},
    inscription::parse_inscription_data,
    utils::generate_taproot_address,
};

/// kind of transactions can be relevant for rollup node to filter
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum TxFilterRule {
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// InscriptionParser which dictates the structure of inscription.
    RollupInscription(RollupName),
    /// Deposit Request transaction
    DepositRequest(DepositTxParams),
    /// Deposit transaction with deposit config and address
    Deposit(DepositTxParams),
    /// Addresses that are spent to
    SpentToAddrs(Vec<BitcoinAddress>),
    /// Blob ids that are expected
    BlobIds(Vec<Buf32>),
    /// Outpoints
    Outpoints(Vec<(Buf32, u32)>),
}

#[derive(Clone, Debug)]
pub struct TxFilterConfig {
    /// For checkpoint update inscriptions.
    rollup_name: RollupName,

    /// For addresses that we expect spends to.
    expected_addrs: Vec<BitcoinAddress>,

    /// For blobs we expect to be written.
    expected_blobs: Vec<Buf32>,

    /// For deposits that might be spent from.
    expected_outpoints: Vec<(Buf32, u32)>,
    // eventually, in future version
    // For bridge V1 deposits that do bitmap flagging for the multisig addr.
    // operator_key_tbl: PublickeyTable,
    /// EE addr length
    ee_addr_len: u8,

    /// Deposit denomination
    deposit_denomination: u64, // sats

    /// Operators addr
    operator_addr: BitcoinAddress,
}

impl TxFilterConfig {
    pub fn from_rollup_params(rollup_params: &RollupParams) -> anyhow::Result<Self> {
        let operator_wallet_pks = get_operator_wallet_pks(rollup_params);
        let address = generate_taproot_address(&operator_wallet_pks, rollup_params.network)?;

        let rollup_name = rollup_params.rollup_name.clone();
        let expected_blobs = Vec::new(); // TODO: this should come from chainstate
        let expected_addrs = vec![address.clone()];
        let expected_outpoints = Vec::new();

        Ok(Self {
            rollup_name,
            expected_blobs,
            expected_addrs,
            expected_outpoints,
            ee_addr_len: rollup_params.address_length,
            deposit_denomination: rollup_params.deposit_amount,
            operator_addr: address,
        })
    }

    pub fn into_rules(self) -> Vec<TxFilterRule> {
        let deposit_params = DepositTxParams {
            magic_bytes: self.rollup_name.clone().into_bytes().to_vec(),
            address_length: self.ee_addr_len,
            deposit_amount: self.deposit_denomination,
            address: self.operator_addr,
        };
        vec![
            TxFilterRule::RollupInscription(self.rollup_name.clone()),
            TxFilterRule::SpentToAddrs(self.expected_addrs),
            TxFilterRule::BlobIds(self.expected_blobs),
            TxFilterRule::Outpoints(self.expected_outpoints),
            TxFilterRule::Deposit(deposit_params.clone()),
            TxFilterRule::DepositRequest(deposit_params),
        ]
    }
}

type RollupName = String;

/// Reads the operator wallet public keys from Rollup params. Returns None if
/// not yet bootstrapped
/// FIXME: This is only for devnet as these pks have to be read from the chain state
fn get_operator_wallet_pks(params: &RollupParams) -> Vec<Buf32> {
    let OperatorConfig::Static(operator_table) = &params.operator_config;

    operator_table.iter().map(|op| *op.wallet_pk()).collect()
}

pub fn derive_tx_filter_rules(params: &RollupParams) -> anyhow::Result<Vec<TxFilterRule>> {
    let operator_wallet_pks = get_operator_wallet_pks(params);
    let address = generate_taproot_address(&operator_wallet_pks, params.network)?;
    let deposit_provider = params.get_deposit_params(address);
    Ok(vec![
        TxFilterRule::RollupInscription(params.rollup_name.clone()),
        TxFilterRule::DepositRequest(deposit_provider.clone()),
        TxFilterRule::Deposit(deposit_provider),
    ])
}

/// Filter protocol operatios as refs from relevant [`Transaction`]s in a block based on given
/// [`TxFilterRule`]s
pub fn filter_protocol_op_tx_refs(
    block: &Block,
    filter_config: TxFilterConfig,
) -> Vec<ProtocolOpTxRef> {
    let filter_rules = filter_config.into_rules();
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            extract_protocol_op(tx, &filter_rules)
                .map(|relevant_tx| ProtocolOpTxRef::new(i as u32, relevant_tx))
        })
        .collect()
}

///  if a [`Transaction`] is relevant based on given [`RelevantTxType`]s then we extract relevant
///  info
fn extract_protocol_op(tx: &Transaction, filters: &[TxFilterRule]) -> Option<ProtocolOperation> {
    filters.iter().find_map(|rel_type| match rel_type {
        TxFilterRule::RollupInscription(name) => tx.input.first().and_then(|inp| {
            inp.witness
                .tapscript()
                .and_then(|scr| parse_inscription_data(&scr.into(), name).ok())
                .and_then(|data| borsh::from_slice::<SignedBatchCheckpoint>(data.batch_data()).ok())
                .map(ProtocolOperation::RollupInscription)
        }),

        TxFilterRule::DepositRequest(config) => extract_deposit_request_info(tx, config)
            .map(|deposit_req_info| Some(ProtocolOperation::DepositRequest(deposit_req_info)))?,

        TxFilterRule::Deposit(config) => extract_deposit_info(tx, config)
            .map(|deposit_info| Some(ProtocolOperation::Deposit(deposit_info)))?,

        // TODO: add others
        _ => None,
    })
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
    use strata_btcio::test_utils::{
        build_reveal_transaction_test, generate_inscription_script_test,
    };
    use strata_primitives::l1::BitcoinAmount;
    use strata_state::{
        batch::SignedBatchCheckpoint,
        tx::{InscriptionData, ProtocolOperation},
    };
    use strata_test_utils::ArbitraryGenerator;

    use crate::{
        deposit::test_utils::{
            build_test_deposit_request_script, build_test_deposit_script,
            create_transaction_two_outpoints, get_deposit_tx_config, test_taproot_addr,
        },
        filter::{filter_protocol_op_tx_refs, TxFilterRule},
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
            filter_protocol_op_tx_refs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
                .iter()
                .map(|op_refs| op_refs.index())
                .collect();

        assert_eq!(txids[0], 0, "Should filter valid rollup name");

        // Test with invalid name
        let rollup_name = "TestRollup".to_string();
        let tx = create_inscription_tx(rollup_name.clone());
        let block = create_test_block(vec![tx]);
        let result = filter_protocol_op_tx_refs(
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
            filter_protocol_op_tx_refs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
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
            filter_protocol_op_tx_refs(&block, &[TxFilterRule::RollupInscription(rollup_name)])
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
            Amount::from_sat(config.deposit_amount),
            &config.address.address().script_pubkey(),
            &deposit_script,
        );

        let block = create_test_block(vec![tx]);

        let filters = vec![TxFilterRule::Deposit(config.clone())];
        let result = filter_protocol_op_tx_refs(&block, &filters);

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
                BitcoinAmount::from_sat(config.deposit_amount),
                "Deposit amount should match"
            );
        } else {
            panic!("Expected Deposit info");
        }
    }

    #[test]
    fn test_filter_relevant_txs_deposit_request() {
        let mut config = get_deposit_tx_config();
        let extra_amt = 10000;
        config.deposit_amount += extra_amt;
        let dest_addr = vec![2u8; 20]; // Example EVM address
        let dummy_block = [0u8; 32]; // Example dummy block
        let deposit_request_script = build_test_deposit_request_script(
            config.magic_bytes.clone(),
            dummy_block.to_vec(),
            dest_addr.clone(),
        );

        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_amount), // Any amount
            &test_taproot_addr().address().script_pubkey(),
            &deposit_request_script,
        );

        let block = create_test_block(vec![tx]);

        let relevant_types = vec![TxFilterRule::DepositRequest(config.clone())];
        let result = filter_protocol_op_tx_refs(&block, &relevant_types);

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
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &ScriptBuf::new(),
        );

        let block = create_test_block(vec![irrelevant_tx]);

        let relevant_types = vec![TxFilterRule::Deposit(config)];
        let result = filter_protocol_op_tx_refs(&block, &relevant_types);

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
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &deposit_script1,
        );
        let tx2 = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &deposit_script2,
        );

        let block = create_test_block(vec![tx1, tx2]);

        let relevant_types = vec![TxFilterRule::Deposit(config.clone())];
        let result = filter_protocol_op_tx_refs(&block, &relevant_types);

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
                    BitcoinAmount::from_sat(config.deposit_amount),
                    "Deposit amount should match for transaction {}",
                    i
                );
            } else {
                panic!("Expected Deposit info for transaction {}", i);
            }
        }
    }
}
