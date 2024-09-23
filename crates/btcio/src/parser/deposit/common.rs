use bitcoin::{script::Instructions, Transaction};

use super::{error::DepositParseError, DepositTxConfig};
use crate::parser::utils::next_bytes;

pub struct DepositRequestScriptInfo {
    pub tap_ctrl_blk_hash: [u8; 32],
    pub ee_bytes: Vec<u8>,
}

/// if the transaction's 0th index is p2tr and amount exceeds the deposit quantity or not
pub fn check_bridge_offer_output<'a>(tx: &'a Transaction, config: &DepositTxConfig) -> bool {
    let txout = &tx.output[0];
    txout.script_pubkey.is_p2tr() && txout.value.to_sat() >= config.deposit_quantity
}

/// check if magic bytes(unique set of bytes used to identify relevant tx) is present or not
pub fn check_magic_bytes(
    instructions: &mut Instructions,
    config: &DepositTxConfig,
) -> Result<(), DepositParseError> {
    // magic bytes
    if let Some(magic_bytes) = next_bytes(instructions) {
        if magic_bytes != config.magic_bytes {
            return Err(DepositParseError::MagicBytesMismatch(
                magic_bytes,
                config.magic_bytes.clone(),
            ));
        }
        return Ok(());
    }

    Err(DepositParseError::NoMagicBytes)
}

/// extracts the Execution environment bytes(most possibly EVM bytes)
pub fn extract_ee_bytes(
    instructions: &mut Instructions,
    config: &DepositTxConfig,
) -> Result<Vec<u8>, DepositParseError> {
    match next_bytes(instructions) {
        Some(ee_bytes) => {
            if ee_bytes.len() as u8 != config.address_length {
                return Err(DepositParseError::InvalidDestAddress(ee_bytes.len() as u8));
            }
            Ok(ee_bytes)
        }
        None => Err(DepositParseError::NoDestAddress),
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{
        opcodes::all::OP_RETURN,
        script::{Builder, PushBytesBuf},
        Amount, ScriptBuf,
    };

    use super::*;
    use crate::parser::deposit::{
        common::{check_bridge_offer_output, check_magic_bytes},
        test_utils::{
            create_transaction_two_outpoints, generic_taproot_addr, get_deposit_tx_config,
        },
    };

    #[test]
    fn test_check_bridge_offer_output_valid() {
        let config = get_deposit_tx_config();
        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &generic_taproot_addr().script_pubkey(),
            &ScriptBuf::new(),
        );

        let result = check_bridge_offer_output(&tx, &config);
        assert!(result);
    }

    #[test]
    fn test_check_bridge_offer_output_invalid() {
        let config = get_deposit_tx_config();
        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity + 1),
            &ScriptBuf::new(),
            &ScriptBuf::new(),
        );

        let result = check_bridge_offer_output(&tx, &config);
        assert!(!result);
    }

    #[test]
    fn test_check_magic_bytes_valid() {
        let config = get_deposit_tx_config();
        let script = Builder::new()
            .push_slice(PushBytesBuf::try_from(config.magic_bytes.clone()).unwrap())
            .push_opcode(OP_RETURN)
            .into_script();
        let mut instructions = script.instructions();

        let result = check_magic_bytes(&mut instructions, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_magic_bytes_invalid() {
        let config = get_deposit_tx_config();
        let script = Builder::new()
            .push_slice(
                PushBytesBuf::try_from("wrong_magic".to_string().as_bytes().to_vec()).unwrap(),
            )
            .push_opcode(OP_RETURN)
            .into_script();
        let mut instructions = script.instructions();

        let result = check_magic_bytes(&mut instructions, &config);
        assert!(matches!(
            result,
            Err(DepositParseError::MagicBytesMismatch(_, _))
        ));
    }

    #[test]
    fn test_check_magic_bytes_missing() {
        let config = get_deposit_tx_config();
        let script = Builder::new().push_opcode(OP_RETURN).into_script();
        let mut instructions = script.instructions();

        let result = check_magic_bytes(&mut instructions, &config);
        assert!(matches!(result, Err(DepositParseError::NoMagicBytes)));
    }

    #[test]
    fn test_extract_ee_bytes_valid() {
        let config = get_deposit_tx_config();
        let ee_bytes = vec![0; config.address_length as usize];
        let script = Builder::new()
            .push_slice(PushBytesBuf::try_from(ee_bytes.clone()).unwrap())
            .push_opcode(OP_RETURN)
            .into_script();
        let mut instructions = script.instructions();

        let result = extract_ee_bytes(&mut instructions, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ee_bytes);
    }

    #[test]
    fn test_extract_ee_bytes_invalid_length() {
        let config = get_deposit_tx_config();
        let ee_bytes = vec![0; (config.address_length as usize) + 1];
        let script = Builder::new()
            .push_slice(PushBytesBuf::try_from(ee_bytes.clone()).unwrap())
            .push_opcode(OP_RETURN)
            .into_script();
        let mut instructions = script.instructions();

        let result = extract_ee_bytes(&mut instructions, &config);
        assert!(matches!(
            result,
            Err(DepositParseError::InvalidDestAddress(_))
        ));
    }

    #[test]
    fn test_extract_ee_bytes_missing() {
        let config = get_deposit_tx_config();
        let script = Builder::new().push_opcode(OP_RETURN).into_script();
        let mut instructions = script.instructions();

        let result = extract_ee_bytes(&mut instructions, &config);
        assert!(matches!(result, Err(DepositParseError::NoDestAddress)));
    }
}
