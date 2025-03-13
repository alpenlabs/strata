//! parser types for Deposit Tx, and later deposit Request Tx

use bitcoin::{opcodes::all::OP_RETURN, OutPoint, ScriptBuf, Transaction};
use strata_bridge_tx_builder::prelude::BRIDGE_DENOMINATION;
use strata_primitives::{
    l1::{DepositInfo, OutputRef},
    prelude::DepositTxParams,
};

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

    // Check if it is exact BRIDGE_DENOMINATION amount
    // TODO make this not a const!
    if send_addr_out.value.to_sat() != BRIDGE_DENOMINATION.to_sat() {
        return None;
    }

    // Check if deposit output address matches.
    if send_addr_out.script_pubkey != config.address.address().script_pubkey() {
        return None;
    }

    // Parse the tag from the OP_RETURN output.
    let tag_data = parse_tag_script(&op_return_out.script_pubkey, config).ok()?;

    // Get the first input of the transaction
    let deposit_outpoint = OutputRef::from(OutPoint {
        txid: tx.compute_txid(),
        vout: 0, // deposit must always exist in the first output
    });

    // Construct and return the DepositInfo
    Some(DepositInfo {
        deposit_idx: tag_data.deposit_idx,
        amt: send_addr_out.value.into(),
        address: tag_data.dest_buf.to_vec(),
        outpoint: deposit_outpoint,
    })
}

struct DepositTag<'buf> {
    deposit_idx: u32,
    dest_buf: &'buf [u8],
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn parse_tag_script<'a>(
    script: &'a ScriptBuf,
    config: &DepositTxParams,
) -> Result<DepositTag<'a>, DepositParseError> {
    let mut instructions = script.instructions();

    // Check if OP_RETURN is present and if not just discard it.
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::MissingTag);
    }

    // Extract the data from the next push.
    let Some(data) = next_bytes(&mut instructions) else {
        return Err(DepositParseError::NoData);
    };

    // If it's not a standard tx then something is *probably* up.
    if data.len() > 80 {
        return Err(DepositParseError::TagOversized);
    }

    parse_tag(data, config)
}

fn parse_tag<'b>(
    buf: &'b [u8],
    config: &DepositTxParams,
) -> Result<DepositTag<'b>, DepositParseError> {
    // data has expected magic bytes
    let magic_bytes = &config.magic_bytes;
    let magic_len = magic_bytes.len();

    // Do some math to make sure there is a magic.
    let exp_min_len = magic_len + 4;
    if buf.len() < exp_min_len {
        return Err(DepositParseError::InvalidMagic);
    }

    let magic_slice = &buf[..magic_len];
    if magic_slice != magic_bytes {
        return Err(DepositParseError::InvalidMagic);
    }

    // Extract the deposit idx.
    let di_buf = &buf[magic_len..exp_min_len];
    let deposit_idx = u32::from_be_bytes([di_buf[0], di_buf[1], di_buf[2], di_buf[3]]);

    // The rest of the buffer is the dest.
    let dest_buf = &buf[exp_min_len..];

    // TODO make this variable sized
    if dest_buf.len() != config.address_length as usize {
        // casting is safe as address.len() < data.len() < 80
        return Err(DepositParseError::InvalidDestLen(dest_buf.len() as u8));
    }

    Ok(DepositTag {
        deposit_idx,
        dest_buf,
    })
}

#[cfg(test)]
mod tests {

    use bitcoin::Amount;
    use strata_test_utils::bitcoin::{
        build_test_deposit_script, create_test_deposit_tx, test_taproot_addr,
    };

    use crate::deposit::{deposit_tx::extract_deposit_info, test_utils::get_deposit_tx_config};

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let config = get_deposit_tx_config();
        let amt = Amount::from_sat(config.deposit_amount);
        let idx = 0xdeadbeef;
        let ee_addr = [1; 20];

        let deposit_request_script =
            build_test_deposit_script(config.magic_bytes, idx, ee_addr.to_vec());

        let test_transaction = create_test_deposit_tx(
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.into());
        assert_eq!(out.deposit_idx, idx);
        assert_eq!(out.address, ee_addr);
    }
}
