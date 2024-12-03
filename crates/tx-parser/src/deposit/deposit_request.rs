//! parser types for Deposit Tx, and later deposit Request Tx

use std::convert::TryInto;

use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction};
use strata_bridge_tx_builder::prelude::BRIDGE_DENOMINATION;
use strata_primitives::params::DepositTxParams;
use strata_state::tx::DepositRequestInfo;
use tracing::debug;

use super::{common::DepositRequestScriptInfo, error::DepositParseError};
use crate::utils::{next_bytes, next_op};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_request_info(
    tx: &Transaction,
    config: &DepositTxParams,
) -> Option<DepositRequestInfo> {
    // Ensure that the transaction has at least 2 outputs
    let addr_txn = tx.output.first()?;
    let op_return_txn = tx.output.get(1)?;

    // Parse the deposit request script from the second output's script_pubkey
    let DepositRequestScriptInfo {
        tap_ctrl_blk_hash,
        ee_bytes,
    } = parse_deposit_request_script(&op_return_txn.script_pubkey, config).ok()?;

    // if sent value is less than equal to what we expect for bridge denomination. The extra amount
    // is used for fees to create deposit transaction
    if addr_txn.value.to_sat() <= BRIDGE_DENOMINATION.to_sat() {
        return None;
    }

    // Construct and return the DepositRequestInfo
    Some(DepositRequestInfo {
        amt: addr_txn.value.to_sat(),
        address: ee_bytes,
        take_back_leaf_hash: tap_ctrl_blk_hash,
    })
}

/// extracts the tapscript block and EE address given that the script is OP_RETURN type and
/// contains the Magic Bytes
pub fn parse_deposit_request_script(
    script: &ScriptBuf,
    config: &DepositTxParams,
) -> Result<DepositRequestScriptInfo, DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        // Commented out these logs since they're really verbose and not
        // helpful.  We shouldn't be emitting a log message for every single tx
        // we see on chain.
        //debug!(?instructions, "missing op_return");
        return Err(DepositParseError::NoOpReturn);
    }

    let Some(data) = next_bytes(&mut instructions) else {
        //debug!("no data after OP_RETURN");
        return Err(DepositParseError::NoData);
    };

    // Added a cfg to assert since it feels like it could crash us in
    // production.  I believe this is just a tx standardness policy, not a
    // consensus rule.
    #[cfg(debug_assertions)]
    assert!(data.len() < 80);

    // data has expected magic bytes
    let magic_bytes = &config.magic_bytes;
    let magic_len = magic_bytes.len();
    if data.len() < magic_len || &data[..magic_len] != magic_bytes {
        return Err(DepositParseError::MagicBytesMismatch);
    }

    // 32 bytes of control hash
    let data = &data[magic_len..];
    if data.len() < 32 {
        //debug!(?data, expected = 32, got = %data.len(), "incorrect number of bytes in hash");
        return Err(DepositParseError::LeafHashLenMismatch);
    }
    let ctrl_hash: &[u8; 32] = &data[..32]
        .try_into()
        .expect("data length must be greater than 32");

    // configured bytes for address
    let address = &data[32..];
    if address.len() != config.address_length as usize {
        // casting is safe as address.len() < data.len() < 80
        debug!(?data, expected = config.address_length, got = %address.len(), "incorrect number of bytes in address");
        return Err(DepositParseError::InvalidDestAddress(address.len() as u8));
    }

    Ok(DepositRequestScriptInfo {
        tap_ctrl_blk_hash: *ctrl_hash,
        ee_bytes: address.into(),
    })
}

#[cfg(test)]
mod tests {
    use bitcoin::{absolute::LockTime, Amount, Transaction};

    use super::extract_deposit_request_info;
    use crate::deposit::{
        deposit_request::parse_deposit_request_script,
        error::DepositParseError,
        test_utils::{
            build_no_op_deposit_request_script, build_test_deposit_request_script,
            create_test_deposit_tx, get_deposit_tx_config, test_taproot_addr,
        },
    };

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let mut config = get_deposit_tx_config();
        let extra_amt = 100000;
        config.deposit_amount += extra_amt;
        let amt = Amount::from_sat(config.deposit_amount);
        let evm_addr = [1; 20];
        let dummy_control_block = [0xFF; 32];
        let test_taproot_addr = test_taproot_addr();

        let deposit_request_script = build_test_deposit_request_script(
            config.magic_bytes.clone(),
            dummy_control_block.to_vec(),
            evm_addr.to_vec(),
        );

        let test_transaction = create_test_deposit_tx(
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr.address().script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_request_info(&test_transaction, &config);

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
        assert_eq!(out.take_back_leaf_hash, dummy_control_block);
    }

    #[test]
    fn test_invalid_script_no_op_return() {
        let evm_addr = [1; 20];
        let control_block = [0xFF; 65];

        let config = get_deposit_tx_config();
        let invalid_script = build_no_op_deposit_request_script(
            config.magic_bytes.clone(),
            control_block.to_vec(),
            evm_addr.to_vec(),
        );

        let out = parse_deposit_request_script(&invalid_script, &config);

        // Should return an error as there's no OP_RETURN
        assert!(matches!(out, Err(DepositParseError::NoOpReturn)));
    }

    #[test]
    fn test_invalid_evm_address_length() {
        let evm_addr = [1; 13]; // Invalid length EVM address
        let control_block = [0xFF; 32];

        let config = get_deposit_tx_config();

        let script = build_test_deposit_request_script(
            config.magic_bytes.clone(),
            control_block.to_vec(),
            evm_addr.to_vec(),
        );
        let out = parse_deposit_request_script(&script, &config);

        // Should return an error as EVM address length is invalid
        assert!(matches!(out, Err(DepositParseError::InvalidDestAddress(_))));
    }

    #[test]
    fn test_invalid_control_block() {
        let evm_addr = [1; 20];
        let control_block = [0xFF; 0]; // Missing control block

        let config = get_deposit_tx_config();
        let script_missing_control = build_test_deposit_request_script(
            config.magic_bytes.clone(),
            control_block.to_vec(),
            evm_addr.to_vec(),
        );

        let out = parse_deposit_request_script(&script_missing_control, &config);

        // Should return an error due to missing control block
        assert!(matches!(out, Err(DepositParseError::LeafHashLenMismatch)));
    }

    #[test]
    fn test_script_with_invalid_magic_bytes() {
        let evm_addr = [1; 20];
        let control_block = vec![0xFF; 32];
        let invalid_magic_bytes = vec![0x00; 4]; // Invalid magic bytes

        let config = get_deposit_tx_config();
        let invalid_script = build_test_deposit_request_script(
            invalid_magic_bytes,
            control_block,
            evm_addr.to_vec(),
        );

        let out = parse_deposit_request_script(&invalid_script, &config);

        // Should return an error due to invalid magic bytes
        assert!(matches!(out, Err(DepositParseError::MagicBytesMismatch)));
    }

    #[test]
    fn test_empty_transaction() {
        let config = get_deposit_tx_config();

        // Empty transaction with no outputs
        let test_transaction = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: LockTime::ZERO,
            input: vec![],
            output: vec![],
        };

        let out = extract_deposit_request_info(&test_transaction, &config);

        // Should return an error as the transaction has no outputs
        assert!(out.is_none());
    }
}
