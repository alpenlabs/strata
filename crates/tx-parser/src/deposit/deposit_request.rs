//! parser types for Deposit Tx, and later deposit Request Tx

use std::convert::TryInto;

use alpen_express_state::tx::DepositReqeustInfo;
use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction};

use super::{
    common::{
        check_bridge_offer_output, check_magic_bytes, extract_ee_bytes, DepositRequestScriptInfo,
    },
    error::DepositParseError,
    DepositTxConfig,
};
use crate::utils::{next_bytes, next_op};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_request_info(
    tx: &Transaction,
    config: &DepositTxConfig,
) -> Option<DepositReqeustInfo> {
    // tapscript output and OP_RETURN must be present
    if tx.output.len() >= 2 {
        if let Ok(DepositRequestScriptInfo {
            tap_ctrl_blk_hash,
            ee_bytes,
        }) = parse_deposit_request_script(&tx.output[1].script_pubkey, config)
        {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if check_bridge_offer_output(tx, config).is_ok() {
                return Some(DepositReqeustInfo {
                    amt: tx.output[0].value.to_sat(),
                    address: ee_bytes,
                    tap_ctrl_blk_hash,
                });
            }
        }
    }
    None
}

/// extracts the tapscript block and EE address given that the script is OP_RETURN type and
/// contains the Magic Bytes
pub fn parse_deposit_request_script(
    script: &ScriptBuf,
    config: &DepositTxConfig,
) -> Result<DepositRequestScriptInfo, DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    check_magic_bytes(&mut instructions, config)?;

    match next_bytes(&mut instructions) {
        Some(ctrl_hash) => {
            // length of control block is 32
            let ee_bytes = extract_ee_bytes(&mut instructions, config)?.to_vec();
            Ok(DepositRequestScriptInfo {
                tap_ctrl_blk_hash: ctrl_hash
                    .try_into()
                    .map_err(|_| DepositParseError::LeafHashLenMismatch)?,
                ee_bytes,
            })
        }
        None => Err(DepositParseError::NoLeafHash),
    }
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
            create_transaction_two_outpoints, generic_taproot_addr, get_deposit_tx_config,
        },
    };

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let config = get_deposit_tx_config();
        let amt = Amount::from_sat(config.deposit_quantity);
        let evm_addr = [1; 20];
        let dummy_control_block = [0xFF; 32];
        let generic_taproot_addr = generic_taproot_addr();

        let deposit_request_script = build_test_deposit_request_script(
            config.magic_bytes,
            dummy_control_block.to_vec(),
            evm_addr.to_vec(),
        );

        let test_transaction = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr.script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_request_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
        assert_eq!(out.tap_ctrl_blk_hash, dummy_control_block);
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
        let control_block = vec![0xFF; 65];
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
