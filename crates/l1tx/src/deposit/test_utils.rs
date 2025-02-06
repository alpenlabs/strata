use strata_primitives::params::DepositTxParams;
use strata_test_utils::bitcoin::test_taproot_addr;

pub fn get_deposit_tx_config() -> DepositTxParams {
    DepositTxParams {
        magic_bytes: "stratasss".to_string().as_bytes().to_vec(),
        address_length: 20,
        deposit_amount: 1_000_000_000,
        address: test_taproot_addr(),
    }
}
