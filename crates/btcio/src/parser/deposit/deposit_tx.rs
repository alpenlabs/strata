//! parser types for Deposit Tx, and later deposit Request Tx

use alpen_express_primitives::tx::DepositInfo;
use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction};

use super::{
    common::{check_magic_bytes, extract_ee_bytes, parse_bridge_offer_output},
    error::DepositParseError,
    DepositTxConfig,
};
use crate::parser::utils::next_op;

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_info(tx: &Transaction, config: &DepositTxConfig) -> Option<DepositInfo> {
    for output in tx.output.iter() {
        if let Ok(ee_address) = parse_deposit_script(&output.script_pubkey, config) {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if let Some((index, tx_out)) = parse_bridge_offer_output(tx, config) {
                return Some(DepositInfo {
                    amt: tx_out.value.to_sat(),
                    deposit_outpoint: index as u32,
                    address: ee_address,
                });
            }
        }
    }
    None
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn parse_deposit_script(
    script: &ScriptBuf,
    config: &DepositTxConfig,
) -> Result<Vec<u8>, DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    // magic bytes
    check_magic_bytes(&mut instructions, config)?;

    // EE address
    extract_ee_bytes(&mut instructions, config)
}

#[cfg(test)]
mod tests {

    use bitcoin::Amount;

    use crate::parser::deposit::{
        deposit_tx::extract_deposit_info,
        test_utils::{
            build_test_deposit_script, create_transaction_two_outpoints, generic_taproot_addr,
            get_deposit_tx_config,
        },
    };

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let amt = Amount::from_sat(1000);
        let evm_addr = [1; 20];
        let generic_taproot_addr = generic_taproot_addr();

        let config = get_deposit_tx_config();
        let deposit_request_script =
            build_test_deposit_script(config.magic_bytes, evm_addr.to_vec());

        let test_transaction = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr.script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
    }
}
