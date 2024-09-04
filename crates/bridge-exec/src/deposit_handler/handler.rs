//! Defines the functions that pertain to handling a deposit.

use bitcoin::secp256k1::schnorr::Signature;
use express_bridge_txm::{DepositInfo, SignatureInfo};

use super::errors::DepositExecResult;

/// Construct and sign the deposit transaction.
pub async fn sign_deposit_tx(_deposit_info: &DepositInfo) -> DepositExecResult<SignatureInfo> {
    unimplemented!()
}

/// Add the signature to the already accumulated set of signatures for a deposit transaction and
/// produce the aggregated signature if all operators have signed. Also update the database
/// entry with the signatures accumulated so far.
//
// TODO: this method will also accept a `BridgeMessage` that holds the signature attached to a
// particular deposit info by other operators.
pub async fn aggregate_signature() -> DepositExecResult<Option<Signature>> {
    unimplemented!()
}
