//! parser types for Deposit Tx, and later deposit Request Tx

use alpen_express_primitives::tx::DepositReqeustInfo;
use bitcoin::{opcodes::all::OP_RETURN, script::Instructions, ScriptBuf, Transaction};

use super::{common::{check_magic_bytes, parse_bridge_offer_output, TapBlkAndAddr}, error::DepositParseError, DepositTxConfig};
use crate::parser::utils::{next_bytes, next_op};

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_request_info(
    tx: &Transaction,
    config: &DepositTxConfig,
) -> Result<DepositReqeustInfo, DepositParseError> {
    for output in tx.output.iter() {
        if let Ok((tap_blk, ee_address)) =
            extract_tapscript_block_and_ee_address(&output.script_pubkey, config)
        {
            // find the outpoint with taproot address, so that we can extract sent amount from that
            if let Some((index, _)) = parse_bridge_offer_output(tx, config) {
                return Ok(DepositReqeustInfo {
                    amt: tx.output[index].value.to_sat(),
                    deposit_outpoint: index as u32,
                    address: ee_address,
                    control_block: tap_blk,
                });
            }
        }
    }

    Err(DepositParseError::NoAddress)
}

/// extracts the taprscript block and EE address given that the script is OP_RETURN type and
/// contains the Magic Bytes
fn extract_tapscript_block_and_ee_address(
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
            extract_ee_bytes(taproot_spend_info, &mut instructions, config)
        }
        None => return Err(DepositParseError::NoControlBlock),
    }
}

fn extract_ee_bytes(taproot_spend_info: Vec<u8>,instructions: &mut Instructions,config: &DepositTxConfig) -> Result<TapBlkAndAddr, DepositParseError>{
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

    use super::{extract_deposit_request_info, DepositTxConfig};

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
        let dummy_hash: [u8; 32] = [0xFF; 32];
        let builder = script::Builder::new()
            .push_opcode(OP_RETURN)
            .push_slice(PushBytesBuf::try_from(alp_magic).unwrap())
            .push_slice(PushBytesBuf::from(dummy_hash))
            .push_slice(PushBytesBuf::try_from(evm_addr.to_vec()).unwrap());

        builder.into_script()
    }

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let amt = Amount::from_sat(1000);
        let evm_addr = [1; 20];
        let dummy_control_block = [0xFF; 32];

        let test_transaction = create_transaction(amt, &evm_addr);
        println!("{:?}", test_transaction.raw_hex());

        let out = extract_deposit_request_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_ok());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.to_sat());
        assert_eq!(out.address, evm_addr);
        assert_eq!(out.control_block, dummy_control_block);
    }
}
