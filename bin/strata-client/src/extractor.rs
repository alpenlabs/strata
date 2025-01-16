//! An extractor extracts data from a relevant source of information.
//!
//! This module abstracts away the data extraction logic from the serving logic in
//! [`super::rpc_server`]. This extraction can happen either from the chain state, the client state
//! or a database.

// This pattern allows testing the actual business logic of the RPCs inside a unit test rather
// than having to worry about all the harnessing that is necessary to test the RPC itself.

use std::sync::Arc;

use bitcoin::{
    hashes::Hash, params::Params, Address, Amount, Network, OutPoint, TapNodeHash, Transaction,
};
use jsonrpsee::core::RpcResult;
use strata_bridge_tx_builder::prelude::{CooperativeWithdrawalInfo, DepositInfo};
use strata_db::traits::L1Database;
use strata_primitives::l1::BitcoinAddress;
use strata_rpc_types::RpcServerError;
use strata_state::{
    bridge_state::{DepositState, DepositsTable},
    tx::ProtocolOperation,
};
use tracing::{debug, error};

/// The `vout` corresponding to the deposit related Taproot address on the Deposit Request
/// Transaction.
///
/// This is always going to be the first [`OutPoint`].
pub const DEPOSIT_REQUEST_VOUT: u32 = 0;

/// Extract the deposit duties from the [`L1Database`] starting from a given block height.
///
/// This duty will be the same for every operator (for now). So, an
/// [`OperatorIdx`](strata_primitives::bridge::OperatorIdx) need not be passed as a
/// parameter.
///
/// # Returns
///
/// A list of [`DepositInfo`] that can be used to construct an equivalent
/// [`Duty`](strata_state::bridge_duties::BridgeDuty).
///
/// # Errors
///
/// If there is an issue accessing entries from the database.
pub(super) async fn extract_deposit_requests<L1DB: L1Database>(
    l1_db: &Arc<L1DB>,
    block_height: u64,
    network: Network,
) -> RpcResult<(impl Iterator<Item = DepositInfo>, u64)> {
    let (l1_txs, latest_idx) = l1_db
        .get_txs_from(block_height)
        .map_err(RpcServerError::Db)?;

    let deposit_info_iter = l1_txs.into_iter().filter_map(move |l1_tx| {
        let tx: Transaction = match l1_tx.tx_data().try_into() {
            Ok(tx) => tx,
            Err(err) => {
                // The client does not benefit from knowing that some transaction stored in the db is
                // invalid/corrupted. They should get the rest of the duties even if some of them are
                // potentially corrupted.
                // The sequencer/full node operator _does_ care about this though. So, we log the
                // error instead.
                error!(%block_height, ?err, "failed to decode raw tx bytes in L1Database");
                debug!(%block_height, ?err, ?l1_tx, "failed to decode raw tx bytes in L1Database");

                return None;
            }
        };

        let deposit_request_outpoint = OutPoint {
            txid: tx.compute_txid(),
            vout: DEPOSIT_REQUEST_VOUT,
        };

        let output_script = &tx
            .output
            .first()
            .expect("an output should exist in the deposit request transaction")
            .script_pubkey;

        let network_params = match network {
            Network::Bitcoin => Params::MAINNET,
            Network::Testnet => Params::TESTNET4,
            Network::Signet => Params::SIGNET,
            Network::Regtest => Params::REGTEST,
            _ => unimplemented!("{} network not handled", network),
        };

        let original_taproot_addr = Address::from_script(output_script, network_params);

        if original_taproot_addr.is_err() {
            // As before, the client does not benefit from knowing that some transaction in the
            // db was stored incorrectly. So, we log the error instead of returning.
            let err = original_taproot_addr.unwrap_err();
            error!(
                ?output_script,
                ?network,
                %block_height,
                ?err,
                "invalid script pubkey output at index 0 in L1Database"
            );
            debug!(%block_height, ?err, ?l1_tx, "invalid script pubkey output at index 0 in L1Database");

            return None;
        }

        let original_taproot_addr = original_taproot_addr
            .expect("script pubkey must produce a valid address for the network");
        let original_taproot_addr =
            BitcoinAddress::parse(&original_taproot_addr.to_string(), network)
                .expect("address generated must be valid for the network");

        if let ProtocolOperation::DepositRequest(info) = l1_tx.protocol_operation() {
            let el_address = info.address.clone();
            let total_amount = Amount::from_sat(info.amt);
            let take_back_leaf_hash = TapNodeHash::from_slice(&info.take_back_leaf_hash)
                .expect("a 32-byte slice must be a valid hash");

            let deposit_info = DepositInfo::new(
                deposit_request_outpoint,
                el_address,
                total_amount,
                take_back_leaf_hash,
                original_taproot_addr,
            );

            return Some(deposit_info);
        }

        None
    });

    Ok((deposit_info_iter, latest_idx))
}

/// Extract the withdrawal duties from the chain state.
///
/// This can be expensive if the chain state has a lot of deposits.
///
/// As this is an internal API, it does need an
/// [`OperatorIdx`](strata_primitives::bridge::OperatorIdx) to be passed in as a withdrawal
/// duty is relevant for all operators for now.
pub(super) fn extract_withdrawal_infos(
    deposits_table: &DepositsTable,
) -> impl Iterator<Item = CooperativeWithdrawalInfo> + '_ {
    let deposits = deposits_table.deposits();

    let withdrawal_infos = deposits.filter_map(|deposit| {
        if let DepositState::Dispatched(dispatched_state) = deposit.deposit_state() {
            let deposit_outpoint = deposit.output().outpoint();
            let user_pk = dispatched_state
                .cmd()
                .withdraw_outputs()
                .first()
                .expect("there should be a withdraw output in a dispatched deposit")
                .dest_addr();
            let assigned_operator_idx = dispatched_state.assignee();
            let exec_deadline = dispatched_state.exec_deadline();

            let withdrawal_info = CooperativeWithdrawalInfo::new(
                *deposit_outpoint,
                *user_pk,
                assigned_operator_idx,
                exec_deadline,
            );

            return Some(withdrawal_info);
        }

        None
    });

    withdrawal_infos
}

#[cfg(test)]
mod tests {
    use std::ops::Not;

    use bitcoin::{
        absolute::LockTime,
        key::rand::{self, Rng},
        opcodes::{OP_FALSE, OP_TRUE},
        script::Builder,
        taproot::LeafVersion,
        transaction::Version,
        ScriptBuf, Sequence, TxIn, TxOut, Witness,
    };
    use rand::rngs::OsRng;
    use strata_bridge_tx_builder::prelude::{create_taproot_addr, SpendPath};
    use strata_common::logging;
    use strata_db::traits::L1Database;
    use strata_mmr::CompactMmr;
    use strata_primitives::{
        bridge::OperatorIdx,
        buf::Buf32,
        l1::{BitcoinAmount, L1BlockManifest, OutputRef, RawBitcoinTx, XOnlyPk},
    };
    use strata_rocksdb::{test_utils::get_rocksdb_tmp_instance, L1Db};
    use strata_state::{
        bridge_state::{
            DepositEntry, DepositsTable, DispatchCommand, DispatchedState, OperatorTable,
            WithdrawOutput,
        },
        chain_state::Chainstate,
        exec_env::ExecEnvState,
        exec_update::UpdateInput,
        genesis::GenesisStateData,
        id::L2BlockId,
        l1::{L1BlockId, L1HeaderRecord, L1Tx, L1TxProof, L1ViewState},
        tx::DepositRequestInfo,
    };
    use strata_test_utils::{bridge::generate_mock_unsigned_tx, ArbitraryGenerator};

    use super::*;

    #[tokio::test]
    async fn test_extract_deposit_requests() {
        // FIXME this is absurd, why are we doing this here?
        logging::init(logging::LoggerConfig::with_base_name("strata-client-tests"));

        let (mock_db, db_config) =
            get_rocksdb_tmp_instance().expect("should be able to get tmp rocksdb instance");

        let l1_db = L1Db::new(mock_db, db_config);
        let l1_db = Arc::new(l1_db);

        let num_blocks = 5;
        const MAX_TXS_PER_BLOCK: usize = 3;

        let (tx, protocol_op, expected_deposit_info) = get_needle();
        let num_valid_duties = populate_db(
            &l1_db.clone(),
            num_blocks,
            MAX_TXS_PER_BLOCK,
            (tx, protocol_op),
        )
        .await;

        let (deposit_infos, latest_idx) =
            extract_deposit_requests(&l1_db.clone(), 0, Network::Regtest)
                .await
                .expect("should be able to extract deposit requests");

        assert_eq!(
            latest_idx,
            (num_blocks - 1) as u64,
            "the latest index returned must the same as the index of the last block"
        );

        let deposit_infos = deposit_infos.collect::<Vec<DepositInfo>>();

        // this is almost always true. In fact, it should be around 50%
        assert!(
            deposit_infos.len() < num_blocks * MAX_TXS_PER_BLOCK,
            "only some txs must have deposit requests"
        );

        assert_eq!(
            deposit_infos.len(),
            num_valid_duties,
            "all the valid duties in the db must be returned"
        );

        let actual_deposit_infos = deposit_infos
            .iter()
            .filter_map(|deposit_info| {
                if expected_deposit_info.eq(deposit_info) {
                    Some(deposit_info.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<DepositInfo>>();

        assert!(
            actual_deposit_infos.len().eq(&1),
            "there should be a known deposit info in the list"
        );
    }

    #[test]
    fn test_extract_withdrawal_infos() {
        let num_deposits = 10;
        let (chain_state, num_withdrawals, needle) =
            generate_empty_chain_state_with_deposits(num_deposits);
        let chain_state = Arc::new(chain_state);

        let withdrawal_infos = extract_withdrawal_infos(chain_state.deposits_table())
            .collect::<Vec<CooperativeWithdrawalInfo>>();

        assert_eq!(
            withdrawal_infos.len(),
            num_withdrawals,
            "number of withdrawals generated and extracted must be the same"
        );

        let deposit_state = needle.deposit_state();
        if let DepositState::Dispatched(dispatched_state) = deposit_state {
            let withdraw_output = dispatched_state
                .cmd()
                .withdraw_outputs()
                .first()
                .expect("should have at least one `WithdrawOutput");
            let user_pk = withdraw_output.dest_addr();

            let expected_info = CooperativeWithdrawalInfo::new(
                *needle.output().outpoint(),
                *user_pk,
                dispatched_state.assignee(),
                dispatched_state.exec_deadline(),
            );

            assert!(
                withdrawal_infos
                    .into_iter()
                    .any(|info| info == expected_info),
                "should be able to find the expected withdrawal info in the list of withdrawal infos"
            );
        } else {
            unreachable!("needle must be in dispatched state");
        }
    }

    /// Populates the db with block data.
    ///
    /// This data includes the `needle` at some random block within the provided range.
    ///
    /// # Returns
    ///
    /// The number of valid deposit request transactions inserted into the db.
    async fn populate_db<Store: L1Database>(
        l1_db: &Arc<Store>,
        num_blocks: usize,
        max_txs_per_block: usize,
        needle: (RawBitcoinTx, ProtocolOperation),
    ) -> usize {
        let mut arb = ArbitraryGenerator::new();
        assert!(
            num_blocks.gt(&0) && max_txs_per_block.gt(&0),
            "num_blocks and max_tx_per_block must be at least 1"
        );

        let random_block = OsRng.gen_range(1..num_blocks);

        let mut num_valid_duties = 0;
        for idx in 0..num_blocks {
            let num_txs = OsRng.gen_range(1..max_txs_per_block);

            let known_tx_idx = if idx == random_block {
                Some(OsRng.gen_range(0..num_txs))
            } else {
                None
            };

            let idx = idx as u64;
            let txs: Vec<L1Tx> = (0..num_txs)
                .map(|i| {
                    let proof = L1TxProof::new(i as u32, arb.generate());

                    // insert the `needle` at the random index in the random block
                    if let Some(known_tx_idx) = known_tx_idx {
                        if known_tx_idx == i {
                            // need clone here because Rust thinks this will be called twice (and
                            // the needle would already have been moved in the second call).
                            num_valid_duties += 1;
                            return L1Tx::new(proof, needle.0.clone(), needle.1.clone());
                        }
                    }

                    let (tx, protocol_op, valid) = generate_mock_tx();
                    if valid {
                        num_valid_duties += 1;
                    }
                    L1Tx::new(proof, tx, protocol_op)
                })
                .collect();

            let mf: L1BlockManifest = arb.generate();

            // Insert block data
            let res = l1_db.put_block_data(idx, mf.clone(), txs.clone());
            assert!(
                res.is_ok(),
                "should be able to put block data into the L1Database"
            );

            // Insert mmr data
            let mmr: CompactMmr = arb.generate();
            l1_db.put_mmr_checkpoint(idx, mmr.clone()).unwrap();
        }

        num_valid_duties
    }

    /// Create a known transaction that should be present in some block.
    fn get_needle() -> (RawBitcoinTx, ProtocolOperation, DepositInfo) {
        let mut arb = ArbitraryGenerator::new();
        let network = Network::Regtest;

        let el_address: [u8; 20] = arb.generate();
        let previous_output: OutputRef = arb.generate();
        let previous_output = *previous_output.outpoint();

        let random_script1 = Builder::new().push_opcode(OP_TRUE).into_script();
        let random_script2 = Builder::new().push_opcode(OP_FALSE).into_script();
        let script2_hash = TapNodeHash::from_script(&random_script2, LeafVersion::TapScript);
        let (taproot_addr, _) = create_taproot_addr(
            &network,
            SpendPath::ScriptSpend {
                scripts: &[random_script1, random_script2],
            },
        )
        .expect("should be able to create a taproot address");

        let num_btc: u64 = 10;
        let tx = Transaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output,
                script_sig: Default::default(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_int_btc(num_btc),
                script_pubkey: taproot_addr.script_pubkey(),
            }],
        };

        let txid = tx.compute_txid();
        let deposit_request_outpoint = OutPoint { txid, vout: 0 };

        let total_amount = num_btc * BitcoinAmount::SATS_FACTOR;
        let protocol_op = ProtocolOperation::DepositRequest(DepositRequestInfo {
            amt: total_amount,
            take_back_leaf_hash: script2_hash.to_byte_array(),
            address: el_address.to_vec(),
        });

        let total_amount = Amount::from_sat(total_amount);
        let expected_deposit_info = DepositInfo::new(
            deposit_request_outpoint,
            el_address.to_vec(),
            total_amount,
            script2_hash,
            BitcoinAddress::parse(&taproot_addr.to_string(), network)
                .expect("address must be valid"),
        );

        (tx.into(), protocol_op, expected_deposit_info)
    }

    /// Generates a mock transaction either arbitrarily or deterministically.
    ///
    /// # Returns
    ///
    /// 1. The encoded mock (unsigned) transaction.
    /// 2. The [`ProtocolOperation::DepositRequest`] corresponding to the transaction.
    /// 3. A flag indicating whether a non-arbitrary [`Transaction`]/[`ProtocolOperation`] pair was
    ///    generated. The chances of an arbitrarily constructed pair being valid is extremely rare.
    ///    So, you can assume that this flag represents whether the pair is valid (true) or invalid
    ///    (false).
    fn generate_mock_tx() -> (RawBitcoinTx, ProtocolOperation, bool) {
        let mut arb = ArbitraryGenerator::new();

        let should_be_valid: bool = arb.generate();

        if should_be_valid.not() {
            let (invalid_tx, invalid_protocol_op) = generate_invalid_tx(&mut arb);
            return (invalid_tx, invalid_protocol_op, should_be_valid);
        }

        let (valid_tx, valid_protocol_op) = generate_valid_tx(&mut arb);
        (valid_tx, valid_protocol_op, should_be_valid)
    }

    fn generate_invalid_tx(arb: &mut ArbitraryGenerator) -> (RawBitcoinTx, ProtocolOperation) {
        let random_protocol_op: ProtocolOperation = arb.generate();

        // true => tx invalid
        // false => script_pubkey in tx output invalid
        let tx_invalid: bool = OsRng.gen_bool(0.5);

        if tx_invalid {
            return (arb.generate(), random_protocol_op);
        }

        let (mut valid_tx, _, _) = generate_mock_unsigned_tx();
        valid_tx.output[0].script_pubkey = ScriptBuf::from_bytes(arb.generate());

        (valid_tx.into(), random_protocol_op)
    }

    fn generate_valid_tx(arb: &mut ArbitraryGenerator) -> (RawBitcoinTx, ProtocolOperation) {
        let (tx, spend_info, script_to_spend) = generate_mock_unsigned_tx();

        let random_hash = *spend_info
            .control_block(&(script_to_spend, LeafVersion::TapScript))
            .expect("should be able to generate control block")
            .merkle_branch
            .as_slice()
            .first()
            .expect("should contain a hash")
            .as_byte_array();

        let deposit_request_info = DepositRequestInfo {
            amt: 1_000_000_000,      // 10 BTC
            address: arb.generate(), // random rollup address (this is fine)
            take_back_leaf_hash: random_hash,
        };

        let deposit_request = ProtocolOperation::DepositRequest(deposit_request_info);

        (tx.into(), deposit_request)
    }

    /// Generate a random chain state with some dispatched deposits.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    ///
    /// * a random empty chain state with some deposits.
    /// * the number of deposits currently dispatched (which is nearly half of all the deposits).
    /// * a random [`DepositEntry`] that has been dispatched.
    fn generate_empty_chain_state_with_deposits(
        num_deposits: usize,
    ) -> (Chainstate, usize, DepositEntry) {
        let l1_block_id = L1BlockId::from(Buf32::zero());
        let safe_block = L1HeaderRecord::new(l1_block_id, vec![], Buf32::zero());
        let l1_state = L1ViewState::new_at_horizon(0, safe_block);

        let operator_table = OperatorTable::new_empty();

        let base_input = UpdateInput::new(0, vec![], Buf32::zero(), vec![]);
        let exec_state = ExecEnvState::from_base_input(base_input, Buf32::zero());

        let l2_block_id = L2BlockId::from(Buf32::zero());
        let gdata = GenesisStateData::new(l2_block_id, l1_state, operator_table, exec_state);

        let mut empty_chain_state = Chainstate::from_genesis(&gdata);

        let empty_deposits = empty_chain_state.deposits_table_mut();
        let mut deposits_table = DepositsTable::new_empty();

        let mut arb = ArbitraryGenerator::new();

        let mut operators: Vec<OperatorIdx> = arb.generate();
        loop {
            if operators.is_empty() {
                operators = arb.generate();
                continue;
            }

            break;
        }

        let random_assignee = OsRng.gen_range(0..operators.len());
        let random_assignee = operators[random_assignee];

        let mut dispatched_deposits = vec![];
        let mut num_dispatched = 0;

        for _ in 0..num_deposits {
            let tx_ref: OutputRef = arb.generate();
            let amt: BitcoinAmount = arb.generate();

            deposits_table.add_deposits(&tx_ref, &operators, amt);

            // dispatch about half of the deposits
            let should_dispatch = OsRng.gen_bool(0.5);
            if should_dispatch.not() {
                continue;
            }

            num_dispatched += 1;

            let random_buf: Buf32 = arb.generate();
            let dest_addr = XOnlyPk::new(random_buf);

            let dispatched_state = DepositState::Dispatched(DispatchedState::new(
                DispatchCommand::new(vec![WithdrawOutput::new(dest_addr, amt)]),
                random_assignee,
                0,
            ));

            let cur_idx = deposits_table.next_idx() - 1;
            let entry = deposits_table.get_deposit_mut(cur_idx).unwrap();

            entry.set_state(dispatched_state);

            dispatched_deposits.push(entry.idx());
        }

        assert!(
            dispatched_deposits.is_empty().not(),
            "some deposits should have been randomly dispatched"
        );

        let needle_index = OsRng.gen_range(0..dispatched_deposits.len());

        let needle = dispatched_deposits
            .get(needle_index)
            .expect("at least one dispatched duty must be present");

        let needle = deposits_table
            .get_deposit(*needle)
            .expect("deposit entry must exist at index")
            .clone();

        *empty_deposits = deposits_table;

        (empty_chain_state, num_dispatched, needle)
    }
}
