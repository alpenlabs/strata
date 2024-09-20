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
    }
    return Err(DepositParseError::NoMagicBytes);
}

