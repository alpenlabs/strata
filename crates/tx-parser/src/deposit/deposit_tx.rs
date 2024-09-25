//! parser types for Deposit Tx, and later deposit Request Tx

use alpen_express_state::tx::DepositInfo;
use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction};

use super::{common::check_bridge_offer_output, error::DepositParseError, DepositTxConfig};
use crate::utils::{next_bytes, next_op};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_info(tx: &Transaction, config: &DepositTxConfig) -> Option<DepositInfo> {
    if tx.output.len() > 1 {
        if let Ok(ee_address) = parse_deposit_script(&tx.output[1].script_pubkey, config) {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if check_bridge_offer_output(tx, config).is_ok() {
                return Some(DepositInfo {
                    amt: tx.output[0].value.to_sat(),
                    deposit_outpoint: 0,
                    address: ee_address.to_vec(),
                });
            }
        }
    }
    None
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn parse_deposit_script<'a>(
    script: &'a ScriptBuf,
    config: &DepositTxConfig,
) -> Result<&'a [u8], DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    let Some(data) = next_bytes(&mut instructions) else {
        return Err(DepositParseError::NoData);
    };

    // data has expected magic bytes
    let magic_bytes = &config.magic_bytes;
    let magic_len = magic_bytes.len();
    if data.len() < magic_len || &data[0..magic_len] != magic_bytes {
        return Err(DepositParseError::MagicBytesMismatch);
    }

    // configured bytes for address
    let address = &data[magic_len..];
    if address.len() != config.address_length as usize {
        return Err(DepositParseError::InvalidDestAddress(address.len() as u8));
    }

    Ok(address)
}

#[cfg(test)]
mod tests {

    use bitcoin::Amount;

    use crate::deposit::{
        deposit_tx::extract_deposit_info,
        test_utils::{
            build_test_deposit_script, create_transaction_two_outpoints, generic_taproot_addr,
            get_deposit_tx_config,
        },
    };

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let config = get_deposit_tx_config();
        let amt = Amount::from_sat(config.deposit_quantity);
        let ee_addr = [1; 20];
        let generic_taproot_addr = generic_taproot_addr();

        let deposit_request_script =
            build_test_deposit_script(config.magic_bytes, ee_addr.to_vec());

        let test_transaction = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr.script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, ee_addr);
    }
}
