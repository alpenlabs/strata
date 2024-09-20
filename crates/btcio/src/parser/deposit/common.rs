use bitcoin::{script::Instructions, Transaction, TxOut};

use crate::parser::utils::next_bytes;

use super::{error::DepositParseError, DepositTxConfig};

pub type TapBlkAndAddr = (Vec<u8>, Vec<u8>);

pub fn parse_bridge_offer_output<'a>(
    tx: &'a Transaction,
    config: &DepositTxConfig,
) -> Option<(usize, &'a TxOut)> {
    tx.output.iter().enumerate().find(|(_, txout)| {
        config
            .federation_address
            .matches_script_pubkey(&txout.script_pubkey)
            && txout.value.to_sat() == config.deposit_quantity
    })
}

pub fn check_magic_bytes(instructions: &mut Instructions,config: &DepositTxConfig) -> Result<(), DepositParseError> {
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

pub fn extract_ee_bytes(instructions: &mut Instructions,config: &DepositTxConfig) -> Result<Vec<u8>, DepositParseError>{
    match next_bytes(instructions) {
        Some(ee_bytes) => {
            if ee_bytes.len() as u8 != config.address_length {
                return Err(DepositParseError::InvalidDestAddress(
                    ee_bytes.len() as u8
                ));
            }
            return Ok(ee_bytes);
        }
        None => {
            return Err(DepositParseError::NoDestAddress);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::deposit::{common::{check_magic_bytes, parse_bridge_offer_output}, test_utils::{create_transaction_two_outpoints, get_deposit_tx_config}};

    use super::*;
    use bitcoin::{opcodes::all::OP_RETURN, script::{Builder, PushBytesBuf}, Amount, ScriptBuf};

    #[test]
    fn test_parse_bridge_offer_output_valid() {
        let config = get_deposit_tx_config();
        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity),
            &config.federation_address.script_pubkey(),
            &ScriptBuf::new()
        );

        let result = parse_bridge_offer_output(&tx, &config);
        assert!(result.is_some());
        let (index, txout) = result.unwrap();
        assert_eq!(index, 0);
        assert_eq!(txout.value.to_sat(), config.deposit_quantity);
        assert_eq!(txout.script_pubkey, config.federation_address.script_pubkey());
    }

    #[test]
    fn test_parse_bridge_offer_output_invalid() {
        let config = get_deposit_tx_config();
        let tx = create_transaction_two_outpoints(
            Amount::from_sat(config.deposit_quantity + 1),
            &ScriptBuf::new(),
            &ScriptBuf::new()
        );

        let result = parse_bridge_offer_output(&tx, &config);
        assert!(result.is_none());
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
            .push_slice(PushBytesBuf::try_from("wrong_magic".to_string().as_bytes().to_vec()).unwrap())
            .push_opcode(OP_RETURN)
            .into_script();
        let mut instructions = script.instructions();

        let result = check_magic_bytes(&mut instructions, &config);
        assert!(matches!(result, Err(DepositParseError::MagicBytesMismatch(_, _))));
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
        assert!(matches!(result, Err(DepositParseError::InvalidDestAddress(_))));
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
