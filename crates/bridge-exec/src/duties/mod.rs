//! This module defines the duties and relevant traits that the bridge client cares about.

use std::{collections::BTreeMap, str::FromStr};

use alpen_express_state::bridge_state::OperatorIdx;
use bitcoin::{
    address::{NetworkChecked, NetworkUnchecked},
    amount::serde::as_sat::deserialize,
    secp256k1::schnorr::Signature,
    Address, Amount, NetworkKind,
};
use express_bridge_txm::{DepositInfo, DepositSignatureInfo, WithdrawalInfo};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Duty {
    SignDeposit {
        deposit_info: DepositInfo,
        signature_info: Option<DepositSignatureInfo>,
    },

    FulfillWithdrawal {
        withdrawal_table: WithdrawalTable,
        assigned_operator_id: OperatorIdx,
        expiry: BitcoinBlockHeight,
    },

    SignWithdrawal {
        withdrawal_info: WithdrawalInfo,
        signature: Option<Signature>,
    },
}

/// The `bitcoin block height`
type BitcoinBlockHeight = usize;

/// The table of withdrawal requests where `Amount` is the requested withdrawal amount and
/// `BitcoinAddress` is the address to deposit the withdrawn amount. We use a `BTreeMap` to preserve
/// the order of withdrawals as that will be used to generate the withdrawal fulfillment transaction
/// chain deterministically.
pub type WithdrawalTable = BTreeMap<Amount, Address>;

// impl FromStr for BitcoinAddress {
//     type Err = String;
//
//     fn from_str(value: &str) -> Result<Self, Self::Err> {
//         Address::from_str(value).map_err(|e| format!("invalid bitcoin address: {e}"))?;
//
//         Ok::<Self, String>(Self(value.to_string()))
//     }
// }
