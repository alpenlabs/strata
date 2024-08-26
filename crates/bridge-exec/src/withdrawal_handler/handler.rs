//! Defines the functions that pertain to handling a withdrawal request.

// TODO: there should be a database that tracks the status of tasks.

use alpen_express_primitives::{bridge::SignatureInfo, l1::BitcoinAmount};
use bitcoin::{address::NetworkChecked, secp256k1::schnorr::Signature, Address, Network, OutPoint};
use express_bridge_tx_builder::withdrawal::CooperativeWithdrawalInfo;

use super::errors::WithdrawalExecResult;

/// Get the outpoint used for front-payments during withdrawal from the supplied reserved
/// address for the given network.
///
/// This involves getting unspent UTXOs in the address and finding an outpoint with enough
/// bitcoins to service the withdrawal via a transaction chain.
// TODO: pass bitcoin rpc client after <https://github.com/alpenlabs/express/pull/251> is merged.
pub fn get_operator_outpoint(
    _reserved_address: Address<NetworkChecked>,
    _network: Network,
    _amount: BitcoinAmount,
) -> OutPoint {
    unimplemented!()
}

/// Sign the reimbursement transaction.
pub async fn sign_reimbursement_tx(
    _withdrawal_info: &CooperativeWithdrawalInfo,
) -> WithdrawalExecResult<SignatureInfo> {
    unimplemented!()
}

/// Aggregate the received signature with the ones already accumulated.
///
/// This is executed by the bridge operator that is assigned the given withdrawal.
// TODO: pass in a database client once the database traits have been implemented.
pub async fn aggregate_withdrawal_sig(
    _withdrawal_info: &CooperativeWithdrawalInfo,
    _sig: &SignatureInfo,
) -> WithdrawalExecResult<Option<Signature>> {
    unimplemented!()
}
