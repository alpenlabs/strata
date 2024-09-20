use std::str::FromStr;

use alpen_express_primitives::params::RollupParams;
use bitcoin::Address;

pub mod common;
pub mod deposit_request;
pub mod deposit_tx;
pub mod error;
pub mod test_utils;

/// Configuration common among deposit and deposit request transaction
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositTxConfig {
    /// magic bytes, usually a rollup name
    pub magic_bytes: Vec<u8>,
    /// EE Address length
    pub address_length: u8,
    /// deposit amount
    pub deposit_quantity: u64,
    /// deposit bridge address
    pub federation_address: Address,
}

impl DepositTxConfig {
    pub fn from_params(params: &RollupParams) -> Self {
        let tap_addr =
            Address::from_str("bcrt1pnmrmugapastum8ztvgwcn8hvq2avmcwh2j4ssru7rtyygkpqq98q4wyd6s")
                .unwrap()
                .require_network(bitcoin::Network::Regtest)
                .unwrap();

        // TODO : have params setup
        Self {
            magic_bytes: params.rollup_name.clone().into_bytes().to_vec(),
            address_length: 10,
            deposit_quantity: 1000000,
            federation_address: tap_addr,
        }
    }

    pub fn new(magic_bytes: &[u8], addr_len: u8, amount: u64, addr: Address) -> Self {
        Self {
            magic_bytes: magic_bytes.to_vec(),
            address_length: addr_len,
            deposit_quantity: amount,
            federation_address: addr,
        }
    }
}
