use std::str::FromStr;

use bitcoin::Address;
use strata_primitives::{l1::BitcoinAddress, params::DepositTxParams};

pub fn test_taproot_addr() -> BitcoinAddress {
    let addr =
        Address::from_str("bcrt1pnmrmugapastum8ztvgwcn8hvq2avmcwh2j4ssru7rtyygkpqq98q4wyd6s")
            .unwrap()
            .require_network(bitcoin::Network::Regtest)
            .unwrap();

    BitcoinAddress::parse(&addr.to_string(), bitcoin::Network::Regtest).unwrap()
}

pub fn get_deposit_tx_config() -> DepositTxParams {
    DepositTxParams {
        magic_bytes: "stratasss".to_string().as_bytes().to_vec(),
        address_length: 20,
        deposit_amount: 1_000_000_000,
        address: test_taproot_addr(),
    }
}
