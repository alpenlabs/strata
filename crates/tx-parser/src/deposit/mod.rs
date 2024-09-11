use alpen_express_primitives::params::RollupParams;
use borsh::{BorshDeserialize, BorshSerialize};

pub mod common;
pub mod deposit_request;
pub mod deposit_tx;
pub mod error;
pub mod test_utils;

/// Configuration common among deposit and deposit request transaction
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct DepositTxConfig {
    /// magic bytes, usually a rollup name
    pub magic_bytes: Vec<u8>,
    /// EE Address length
    pub address_length: u8,
    /// deposit amount
    pub deposit_quantity: u64,
}

impl From<&RollupParams> for DepositTxConfig {
    fn from(params: &RollupParams) -> Self {
        Self {
            magic_bytes: params.rollup_name.clone().into_bytes().to_vec(),
            address_length: params.address_length,
            deposit_quantity: params.deposit_amount,
        }
    }
}

impl DepositTxConfig {
    pub fn new(magic_bytes: &[u8], addr_len: u8, amount: u64) -> Self {
        Self {
            magic_bytes: magic_bytes.to_vec(),
            address_length: addr_len,
            deposit_quantity: amount,
        }
    }
}
