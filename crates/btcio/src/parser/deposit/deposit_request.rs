//! parser types for Deposit Tx, and later deposit Request Tx

use alpen_express_primitives::tx::DepositReqeustInfo;
use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction};

use super::{common::{check_magic_bytes, extract_ee_bytes, parse_bridge_offer_output, TapBlkAndAddr}, error::DepositParseError, DepositTxConfig};
use crate::parser::utils::{next_bytes, next_op};


/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_request_info(
    tx: &Transaction,
    config: &DepositTxConfig,
) -> Option<DepositReqeustInfo> {
    for output in tx.output.iter() {
        if let Ok((tap_blk, ee_address)) =  parse_deposit_request_script(&output.script_pubkey, config) {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if let Some((index, tx_out)) = parse_bridge_offer_output(tx, config) {
                return Some(DepositReqeustInfo {
                    amt: tx_out.value.to_sat(),
                    deposit_outpoint: index as u32,
                    address: ee_address,
                    control_block: tap_blk,
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
) ->Result<TapBlkAndAddr, DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    check_magic_bytes(&mut instructions, config)?;

    match next_bytes(&mut instructions) {
        Some(taproot_spend_info) => {
            // length of control block is 32
            match taproot_spend_info.len() == 32 {
                true => {
                    let ee_bytes = extract_ee_bytes(&mut instructions, config)?;

                    Ok((taproot_spend_info, ee_bytes))
                }
                false => Err(DepositParseError::ControlBlockLen),
            }
        }
        None => Err(DepositParseError::NoControlBlock),
    }
}



#[cfg(test)]
mod tests {
    use bitcoin::{
        absolute::LockTime, opcodes::all::OP_RETURN, script::{self, PushBytesBuf}, Amount, ScriptBuf, Transaction};

    use crate::parser::deposit::{deposit_request::parse_deposit_request_script, error::DepositParseError, test_utils::{create_transaction_two_outpoints, generic_taproot_addr, get_deposit_tx_config}};

    use super::extract_deposit_request_info;

    fn build_no_op_script(magic: Vec<u8>,dummy_block: Vec<u8>, evm_addr: Vec<u8>) -> ScriptBuf {
        let builder = script::Builder::new()
            .push_slice(PushBytesBuf::try_from(magic).unwrap())
            .push_slice(PushBytesBuf::try_from(dummy_block).unwrap())
            .push_slice(PushBytesBuf::try_from(evm_addr).unwrap());

        builder.into_script()
    }

    fn build_test_deposit_script(magic: Vec<u8>,dummy_block: Vec<u8>, evm_addr: Vec<u8>) -> ScriptBuf {
        let builder = script::Builder::new()
            .push_opcode(OP_RETURN)
            .push_slice(PushBytesBuf::try_from(magic).unwrap())
            .push_slice(PushBytesBuf::try_from(dummy_block).unwrap())
            .push_slice(PushBytesBuf::try_from(evm_addr).unwrap());

        builder.into_script()
    }


    #[test]
    fn check_deposit_parser() {

        // values for testing
        let amt = Amount::from_sat(1000);
        let evm_addr = [1; 20];
        let dummy_control_block = [0xFF; 32];
        let generic_taproot_addr = generic_taproot_addr();

        let config = get_deposit_tx_config();
        let deposit_request_script = build_test_deposit_script(config.magic_bytes, dummy_control_block.to_vec(), evm_addr.to_vec());

        let test_transaction = create_transaction_two_outpoints(Amount::from_sat(config.deposit_quantity),&generic_taproot_addr.script_pubkey(), &deposit_request_script);

        let out = extract_deposit_request_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
        assert_eq!(out.control_block, dummy_control_block);
    }

    #[test]
    fn test_invalid_script_no_op_return() {
        let evm_addr = [1; 20];
        let control_block = [0xFF; 65];

        let config = get_deposit_tx_config();
        let invalid_script = build_no_op_script(config.magic_bytes.clone(), control_block.to_vec(), evm_addr.to_vec());

        let out = parse_deposit_request_script(&invalid_script, &config);

        // Should return an error as there's no OP_RETURN
        assert!(matches!(out, Err(DepositParseError::NoOpReturn)));
    }

    #[test]
    fn test_invalid_evm_address_length() {
        let evm_addr = [1; 13]; // Invalid length EVM address
        let control_block = [0xFF; 32];

        let config = get_deposit_tx_config();


        let script = build_test_deposit_script(config.magic_bytes.clone(),control_block.to_vec(), evm_addr.to_vec());
        let out = parse_deposit_request_script(&script, &config);

        // Should return an error as EVM address length is invalid
        assert!(matches!(out, Err(DepositParseError::InvalidDestAddress(_))));
    }

    #[test]
    fn test_invalid_control_block() {
        let evm_addr = [1; 20];
        let control_block = [0xFF; 0]; // Missing control block

        let config = get_deposit_tx_config();
        let script_missing_control = build_test_deposit_script(config.magic_bytes.clone(), control_block.to_vec(), evm_addr.to_vec());


        let out = parse_deposit_request_script(&script_missing_control, &config);

        // Should return an error due to missing control block
        assert!(matches!(out, Err(DepositParseError::ControlBlockLen)));
    }

    #[test]
    fn test_script_with_invalid_magic_bytes() {
        let evm_addr = [1; 20];
        let control_block = [0xFF; 65];
        let invalid_magic_bytes = vec![0x00; 4]; // Invalid magic bytes

        let config = get_deposit_tx_config();
        let invalid_script = build_test_deposit_script(invalid_magic_bytes, control_block.to_vec(), evm_addr.to_vec());

        let out = parse_deposit_request_script(&invalid_script, &config);

        // Should return an error due to invalid magic bytes
        assert!(matches!(out, Err(DepositParseError::MagicBytesMismatch(_,_))));
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
        assert!(matches!(out, None));
    }
}
