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
