//! Defines the functions that pertain to handling a withdrawal request.

use std::sync::Arc;

use bitcoin::{
    address::NetworkChecked, secp256k1::schnorr::Signature, Address, Network,
    OutPoint,
};

use alpen_express_primitives::l1::BitcoinAmount;
use express_bridge_txm::{ReimbursementRequest, SignatureInfo};

use crate::withdrawal_handler::errors::{WithdrawalExecError, WithdrawalExecResult};

/// Get the outpoint used for front-payments during withdrawal from the supplied reserved
/// address for the given network.
///
/// This involves getting unspent UTXOs in the address and finding an outpoint with enough
/// bitcoins to service the withdrawal via a transaction chain.
pub async fn get_operator_outpoint(
    reserved_address: Address<NetworkChecked>,
    network: Network,
    amount: BitcoinAmount,
    rpc_client: Arc<impl rpc_client::BitcoinReader + rpc_client::BitcoinWallet>,
) -> WithdrawalExecResult<OutPoint> {
    let utxos = rpc_client
        .get_utxos()
        .await
        .expect("Could not get UTXOs from the BitcoinWalletRPC") // FIXME: convert to `?` once #251 is merged
        .into_iter()
        .filter(|utxo| utxo.address == reserved_address);
    let candidate_utxo = utxos.find(|utxo| utxo.amount.to_sat() >= amount.to_sat());
    match candidate_utxo {
        Some(utxo) => Ok(OutPoint {
            txid: utxo.txid,
            vout: utxo.vout,
        }),
        None => Err(WithdrawalExecError::InsufficientFunds),
    }
}

/// Sign the reimbursement transaction.
pub async fn sign_reimbursement_tx(
    _withdrawal_info: &ReimbursementRequest,
) -> WithdrawalExecResult<SignatureInfo> {
    unimplemented!()
}

/// Aggregate the received signature with the ones already accumulated.
///
/// This is executed by the bridge operator that is assigned the given withdrawal.
// TODO: pass in a database client once the database traits have been implemented.
pub async fn aggregate_withdrawal_sig(
    _withdrawal_info: &ReimbursementRequest,
    _sig: &SignatureInfo,
) -> WithdrawalExecResult<Option<Signature>> {
    unimplemented!()
}
