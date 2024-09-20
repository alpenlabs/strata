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

pub fn extract_ee_bytes(taproot_spend_info: Vec<u8>,instructions: &mut Instructions,config: &DepositTxConfig) -> Result<TapBlkAndAddr, DepositParseError>{
    match next_bytes(instructions) {
        Some(ee_bytes) => {
            if ee_bytes.len() as u8 != config.address_length {
                return Err(DepositParseError::InvalidDestAddress(
                    ee_bytes.len() as u8
                ));
            }
            return Ok((taproot_spend_info, ee_bytes));
        }
        None => {
            return Err(DepositParseError::NoAddress);
        }
    }
}
