//! parser types for Deposit Tx, and later deposit Request Tx

use alpen_express_primitives::tx::DepositInfo;
use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction, TxOut};

use super::{error::DepositParseError, common::check_magic_bytes, DepositTxConfig};
use crate::parser::utils::{next_bytes, next_op};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_info(
    tx: &Transaction,
    config: &DepositTxConfig,
) -> Result<DepositInfo, DepositParseError> {
    for output in tx.output.iter() {
        if let Ok(ee_address) = extract_ee_address(&output.script_pubkey, config) {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if let Some((index, _)) = parse_bridge_offer_output(tx, config) {
                return Ok(DepositInfo {
                    amt: tx.output[index].value.to_sat(),
                    deposit_outpoint: index as u32,
                    address: ee_address,
                });
            }
        }
    }
    Err(DepositParseError::NoAddress)
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn extract_ee_address(
    script: &ScriptBuf,
    config: &DepositTxConfig,
) -> Result<Vec<u8>, DepositParseError> {
    let mut instructions = script.instructions();

    // check if OP_RETURN is present and if not just discard it
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::NoOpReturn);
    }

    // magic bytes
    check_magic_bytes(&mut instructions, config)?;

    if let Some(ee_bytes) = next_bytes(&mut instructions) {
        if ee_bytes.len() as u8 != config.address_length {
            return Err(DepositParseError::InvalidDestAddress(
                ee_bytes.len() as u8
            ));
        }
        return Ok(ee_bytes);
    }else {
        return Err(DepositParseError::NoAddress);
    }
}

fn parse_bridge_offer_output<'a>(
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

fn check_transaction_amounts(tx: &TxOut, deposit_quantity: u64) -> bool {
    tx.value.to_sat() == deposit_quantity
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitcoin::{
        absolute::LockTime,
        opcodes::all::OP_RETURN,
        script::{self, PushBytesBuf},
        Address, Amount, ScriptBuf, Transaction, TxOut,
    };
    use bitcoind::bitcoincore_rpc::RawTx;

    use super::{extract_deposit_info, DepositTxConfig};

    pub fn taproot_addr() -> Address {
        // Maybe N-of-N Address
        Address::from_str("bcrt1pnmrmugapastum8ztvgwcn8hvq2avmcwh2j4ssru7rtyygkpqq98q4wyd6s")
            .unwrap()
            .require_network(bitcoin::Network::Regtest)
            .unwrap()
    }

    pub fn get_deposit_tx_config() -> DepositTxConfig {
        DepositTxConfig {
            magic_bytes: "expresssss".to_string().as_bytes().to_vec(),
            address_length: 20,
            deposit_quantity: 1000,
            federation_address: taproot_addr(),
        }
    }

    fn create_transaction(amt: Amount, evm_addr: &[u8]) -> Transaction {
        // Construct the outputs
        let outputs = vec![
            TxOut {
                value: amt, // 10 BTC in satoshis
                script_pubkey: taproot_addr().script_pubkey(),
            },
            TxOut {
                value: Amount::ZERO, // Amount is zero for OP_RETURN
                script_pubkey: build_test_deposit_script(evm_addr),
            },
        ];

        // Create the transaction
        Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: LockTime::ZERO,
            input: vec![],
            output: outputs,
        }
    }

    fn build_test_deposit_script(evm_addr: &[u8]) -> ScriptBuf {
        let alp_magic = "expresssss".to_string().as_bytes().to_vec();
        let builder = script::Builder::new()
            .push_opcode(OP_RETURN)
            .push_slice(PushBytesBuf::try_from(alp_magic).unwrap())
            .push_slice(PushBytesBuf::try_from(evm_addr.to_vec()).unwrap());

        builder.into_script()
    }

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let amt = Amount::from_sat(1000);
        let evm_addr = [1; 20];

        let test_transaction = create_transaction(amt, &evm_addr);
        println!("{:?}", test_transaction.raw_hex());

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_ok());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
    }
}
