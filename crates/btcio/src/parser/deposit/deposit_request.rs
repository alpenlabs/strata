//! parser types for Deposit Tx, and later deposit Request Tx

use bitcoin::{opcodes::all::OP_RETURN, ScriptBuf, Transaction, TxOut, Amount};

use crate::parser::utils::{next_bytes, next_op};

use super::{error::DepositParseError, DepositTxConfig};

#[derive(Clone, Debug)]
pub struct DepositReqeustInfo {
    /// amount in satoshis
    pub amt: Amount,

    /// outpoint where amount is present
    pub deposit_outpoint: u32,

    /// tapscript control block for timelock script
    pub control_block: Vec<u8>,

    /// EE address
    pub address: Vec<u8>,
}

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_request_info(
    tx: &Transaction,
    config: &DepositTxConfig,
) -> Result<DepositReqeustInfo, DepositParseError> {
    for output in tx.output.iter() {
        if let Some(Ok((tap_blk,ee_address ))) = extract_tapscript_block_and_ee_address(&output.script_pubkey, config) {
          // find the outpoint with taproot address, so that we can extract sent amount from that
          if let Some((index, _)) = parse_bridge_offer_output(tx, config) {
              return Ok(DepositReqeustInfo {
                amt: tx.output[index].value,
                deposit_outpoint: index as u32,
                address: ee_address,
                control_block: tap_blk,
            })
          }
        }
    }
// check the amount
    // check for validty of n of n valid address

    Err(DepositParseError::NoAddress)
}

/// extracts the taprscript block and EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn extract_tapscript_block_and_ee_address(script: &ScriptBuf, config: &DepositTxConfig) -> Option<Result<(Vec<u8>,Vec<u8>),DepositParseError>> {
        let mut instructions = script.instructions();

        // check if OP_RETURN is present and if not just discard it
        if next_op(&mut instructions) != Some(OP_RETURN) {
            return None;
        }

        // magic bytes
        if let Some(magic_bytes) = next_bytes(&mut instructions) {
            if magic_bytes != config.magic_bytes {
                return Some(Err(DepositParseError::MagicBytesMismatch(
                    magic_bytes,
                    config.magic_bytes.clone(),
                )));
            }
        } else {
            return Some(Err(DepositParseError::NoMagicBytes));
        }

        if let Some(taproot_spend_info) = next_bytes(&mut instructions) {
            if let Some(ee_bytes) = next_bytes(&mut instructions) {
                if ee_bytes.len() as u8 != config.address_length {
                    return Some(Err(DepositParseError::InvalidDestAddress(ee_bytes.len() as u8)));
                }
                return Some(Ok((taproot_spend_info, ee_bytes)))
            }
        }

        None
}

fn parse_bridge_offer_output<'a, 'b>(tx: &'a Transaction, config: &'b DepositTxConfig) -> Option<(usize, &'a TxOut)> {
    tx.output.iter().enumerate().find(|(_, txout)| {
        config.federation_address.matches_script_pubkey(&txout.script_pubkey) && txout.value.to_sat() == config.deposit_quantity
    })
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
            federation_address: taproot_addr()
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
        let dummy_hash: [u8;32] = [0xFF;32];
        let builder = script::Builder::new()
            .push_opcode(OP_RETURN)
            .push_slice(PushBytesBuf::try_from(alp_magic).unwrap())
            .push_slice(PushBytesBuf::try_from(dummy_hash).unwrap())
            .push_slice(PushBytesBuf::try_from(evm_addr.to_vec()).unwrap());

        builder.into_script()
    }

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let amt = Amount::from_sat(1000);
        let evm_addr = [1; 20];
        let dummy_control_block = [0xFF;32];

        let test_transaction = create_transaction(amt, &evm_addr);
        println!("{:?}", test_transaction.raw_hex());

        let out = extract_deposit_request_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_ok());
        let out = out.unwrap();

        assert_eq!(out.amt, amt);
        assert_eq!(out.address, evm_addr);
        assert_eq!(out.control_block, dummy_control_block);
    }
}
