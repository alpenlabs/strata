//! parser types for Deposit Tx, and later deposit Request Tx

use bitcoin::{opcodes::all::OP_RETURN, OutPoint, ScriptBuf, Transaction};
use strata_bridge_tx_builder::prelude::BRIDGE_DENOMINATION;
use strata_primitives::{l1::OutputRef, prelude::DepositTxParams};
use strata_state::tx::DepositInfo;

use crate::{
    deposit::error::DepositParseError,
    utils::{next_bytes, next_op},
};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_info(tx: &Transaction, config: &DepositTxParams) -> Option<DepositInfo> {
    // Get the first output (index 0)
    let send_addr_out = tx.output.first()?;

    // Get the second output (index 1)
    let op_return_out = tx.output.get(1)?;

    // Parse the deposit script from the second output's script_pubkey
    let ee_address = parse_deposit_script(&op_return_out.script_pubkey, config).ok()?;

    // check if it is exact BRIDGE_DENOMINATION amount
    if send_addr_out.value.to_sat() != BRIDGE_DENOMINATION.to_sat() {
        return None;
    }

    // check if p2tr address matches
    if send_addr_out.script_pubkey != config.address.address().script_pubkey() {
        return None;
    }

    // Get the first input of the transaction
    let deposit_outpoint = OutputRef::from(OutPoint {
        txid: tx.compute_txid(),
        vout: 0, // deposit must always exist in the first output
    });

    // Construct and return the DepositInfo
    Some(DepositInfo {
        amt: send_addr_out.value.into(),
        address: ee_address.to_vec(),
        outpoint: deposit_outpoint,
    })
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn parse_deposit_script<'a>(
    script: &'a ScriptBuf,
    config: &DepositTxParams,
) -> Result<&'a [u8], DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    let Some(data) = next_bytes(&mut instructions) else {
        return Err(DepositParseError::NoData);
    };

    assert!(data.len() < 80);

    // data has expected magic bytes
    let magic_bytes = &config.magic_bytes;
    let magic_len = magic_bytes.len();

    if data.len() < magic_len || &data[..magic_len] != magic_bytes {
        return Err(DepositParseError::MagicBytesMismatch);
    }

    // configured bytes for address
    let address = &data[magic_len..];
    if address.len() != config.address_length as usize {
        // casting is safe as address.len() < data.len() < 80
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
            build_test_deposit_script, create_test_deposit_tx, get_deposit_tx_config,
            test_taproot_addr,
        },
    };

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let config = get_deposit_tx_config();
        let amt = Amount::from_sat(config.deposit_amount);
        let ee_addr = [1; 20];

        let deposit_request_script =
            build_test_deposit_script(config.magic_bytes, ee_addr.to_vec());

        let test_transaction = create_test_deposit_tx(
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.into());
        assert_eq!(out.address, ee_addr);
    }
}
