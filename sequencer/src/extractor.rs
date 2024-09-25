//! An extractor extracts data from a relevant source of information.
//!
//! This module abstracts away the data extraction logic from the serving logic in
//! [`super::rpc_server`]. This extraction can happen either from the chain state, the client state
//! or a database.
//!
//! This pattern allows us to test the actual business logic of the RPCs inside a unit test rather
//! than worry about all the harnessing that is necessary to test the RPC itself.

use std::sync::Arc;

use alpen_express_db::traits::L1DataProvider;
use alpen_express_primitives::l1::BitcoinAddress;
use alpen_express_state::tx::ProtocolOperation;
use bitcoin::{
    consensus::Decodable, hashes::Hash, params::Params, Address, Amount, Network, OutPoint,
    TapNodeHash, Transaction,
};
use express_bridge_tx_builder::prelude::DepositInfo;
use jsonrpsee::core::RpcResult;
use tracing::{debug, error};

use crate::errors::RpcServerError;

/// Extract the deposit duties from the [`L1DataProvider`] starting from a given block height.
///
/// This duty will be the same for every operator (for now). So, an
/// [`OperatorIdx`](alpen_express_primitives::bridge::OperatorIdx) need not be passed as a
/// parameter.
///
/// # Returns
///
/// A list of [`DepositInfo`] that can be used to construct an equivalent
/// [`Duty`](alpen_express_state::bridge_duties::Duty).
///
/// # Errors
///
/// If there is an issue accessing entries from the database.
pub(super) async fn extract_deposit_requests<Provider: L1DataProvider>(
    l1_data_provider: &Arc<Provider>,
    block_height: u64,
    network: Network,
) -> RpcResult<impl Iterator<Item = DepositInfo>> {
    let l1_txs = l1_data_provider
        .get_txs_after(block_height)
        .map_err(RpcServerError::Db)?;

    let deposit_info_iter = l1_txs.into_iter().filter_map(move |l1_tx| {
        let mut raw_tx = l1_tx.tx_data();
        let tx = Transaction::consensus_decode(&mut raw_tx);
        if tx.is_err() {
            // The client does not benefit from knowing that some transaction stored in the db is
            // invalid/corrupted. They should get the rest of the duties even if some of them are
            // potentially corrupted.
            // The sequencer/full node operator _does_ care about this though. So, we log the
            // error instead.
            let err = tx.unwrap_err();
            error!(%block_height, ?err, "failed to decode raw tx bytes in L1DataStore");
            debug!(%block_height, ?err, ?l1_tx, "failed to decode raw tx bytes in L1DataStore");

            return None;
        }

        let tx = tx.expect("raw tx bytes must be decodable");

        let deposit_request_outpoint = OutPoint {
            txid: tx.compute_txid(),
            vout: 0, // always the first
        };

        let output_script = &tx
            .output
            .first()
            .expect("an output should exist in the deposit request transaction")
            .script_pubkey;

        let network_params = match network {
            Network::Bitcoin => Params::MAINNET,
            Network::Testnet => Params::TESTNET,
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
                "invalid script pubkey output at index 0 in L1DataStore"
            );
            debug!(%block_height, ?err, ?l1_tx, "invalid script pubkey output at index 0 in L1DataStore");

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

    Ok(deposit_info_iter)
}

#[cfg(test)]
mod tests {
    use std::ops::Not;

    use alpen_express_common::logging;
    use alpen_express_db::traits::L1DataStore;
    use alpen_express_mmr::CompactMmr;
    use alpen_express_primitives::{
        l1::{L1BlockManifest, L1Tx, L1TxProof},
        tx::DepositRequestInfo,
    };
    use alpen_express_rocksdb::{test_utils::get_rocksdb_tmp_instance, L1Db};
    use alpen_test_utils::{bridge::generate_mock_unsigned_tx, ArbitraryGenerator};
    use bitcoin::{
        consensus::Encodable,
        key::rand::{self, Rng},
        taproot::LeafVersion,
        ScriptBuf,
    };

    use super::*;

    #[tokio::test]
    async fn test_extract_deposit_requests() {
        logging::init();

        let (mock_db, db_config) =
            get_rocksdb_tmp_instance().expect("should be able to get tmp rocksdb instance");

        let l1_db = L1Db::new(mock_db, db_config);
        let l1_db = Arc::new(l1_db);

        let num_blocks = 5;
        const MAX_TXS_PER_BLOCK: usize = 3;

        let num_valid_duties = populate_db(&l1_db.clone(), num_blocks, MAX_TXS_PER_BLOCK).await;

        let txs = extract_deposit_requests(&l1_db.clone(), 0, Network::Regtest)
            .await
            .expect("should be able to extract deposit requests")
            .collect::<Vec<DepositInfo>>();

        // this is almost always true. In fact, it should be around 50%
        assert!(
            txs.len() < num_blocks * MAX_TXS_PER_BLOCK,
            "only some txs must have deposit requests"
        );

        assert_eq!(
            txs.len(),
            num_valid_duties,
            "all the valid duties in the db must be returned"
        );
    }

    /// Populates the db with block data.
    ///
    /// # Returns
    ///
    /// The number of valid deposit request transactions inserted into the db.
    async fn populate_db<Store: L1DataStore>(
        l1_db: &Arc<Store>,
        num_blocks: usize,
        max_txs_per_block: usize,
    ) -> usize {
        let arb = ArbitraryGenerator::new();

        let mut num_valid_duties = 0;
        for idx in 0..num_blocks {
            let idx = idx as u64;

            let num_txs = rand::thread_rng().gen_range(0..max_txs_per_block);

            let txs: Vec<L1Tx> = (0..num_txs)
                .map(|i| {
                    let proof = L1TxProof::new(i as u32, arb.generate());
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
                "should be able to put block data into the L1Db"
            );

            // Insert mmr data
            let mmr: CompactMmr = arb.generate();
            l1_db.put_mmr_checkpoint(idx, mmr.clone()).unwrap();
        }

        num_valid_duties
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
    fn generate_mock_tx() -> (Vec<u8>, ProtocolOperation, bool) {
        let arb = ArbitraryGenerator::new();

        let should_be_valid: bool = arb.generate();

        if should_be_valid.not() {
            let (invalid_tx, invalid_protocol_op) = generate_invalid_tx(&arb);
            return (invalid_tx, invalid_protocol_op, should_be_valid);
        }

        let (valid_tx, valid_protocol_op) = generate_valid_tx(&arb);
        (valid_tx, valid_protocol_op, should_be_valid)
    }

    fn generate_invalid_tx(arb: &ArbitraryGenerator) -> (Vec<u8>, ProtocolOperation) {
        let random_protocol_op: ProtocolOperation = arb.generate();

        // true => tx invalid
        // false => script_pubkey in tx output invalid
        let tx_invalid: bool = rand::thread_rng().gen_bool(0.5);

        if tx_invalid {
            let mut random_tx: Vec<u8> = arb.generate();
            while random_tx.is_empty() {
                random_tx = arb.generate();
            }

            return (random_tx, random_protocol_op);
        }

        let (mut valid_tx, _, _) = generate_mock_unsigned_tx();
        valid_tx.output[0].script_pubkey = ScriptBuf::from_bytes(arb.generate());

        let mut tx_with_invalid_script_pubkey = vec![];
        valid_tx
            .consensus_encode(&mut tx_with_invalid_script_pubkey)
            .expect("should be able to encode tx");

        (tx_with_invalid_script_pubkey, random_protocol_op)
    }

    fn generate_valid_tx(arb: &ArbitraryGenerator) -> (Vec<u8>, ProtocolOperation) {
        let (tx, spend_info, script_to_spend) = generate_mock_unsigned_tx();

        let random_hash = *spend_info
            .control_block(&(script_to_spend, LeafVersion::TapScript))
            .expect("should be able to generate control block")
            .merkle_branch
            .as_slice()
            .first()
            .expect("should contain a hash")
            .as_byte_array();

        let mut raw_tx = vec![];
        tx.consensus_encode(&mut raw_tx)
            .expect("should be able to encode transaction");

        let deposit_request_info = DepositRequestInfo {
            amt: 1_000_000_000,      // 10 BTC
            address: arb.generate(), // random rollup address (this is fine)
            tap_ctrl_blk_hash: random_hash,
        };

        let deposit_request = ProtocolOperation::DepositRequest(deposit_request_info);

        (raw_tx, deposit_request)
    }
}
