use std::str::FromStr;

use bitcoin::{
    absolute::LockTime,
    opcodes::all::OP_RETURN,
    script::{self, PushBytesBuf},
    Address, Amount, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use strata_primitives::{
    l1::{BitcoinAddress, OutputRef},
    params::DepositTxParams,
};
use strata_test_utils::ArbitraryGenerator;

pub fn test_taproot_addr() -> BitcoinAddress {
    let addr =
        Address::from_str("bcrt1pnmrmugapastum8ztvgwcn8hvq2avmcwh2j4ssru7rtyygkpqq98q4wyd6s")
            .unwrap()
            .require_network(bitcoin::Network::Regtest)
            .unwrap();

    BitcoinAddress::parse(&addr.to_string(), bitcoin::Network::Regtest).unwrap()
}

pub fn get_deposit_tx_config() -> DepositTxParams {
    DepositTxParams {
        magic_bytes: "stratasss".to_string().as_bytes().to_vec(),
        address_length: 20,
        deposit_amount: 1_000_000_000,
        address: test_taproot_addr(),
    }
}

pub fn create_test_deposit_tx(
    amt: Amount,
    addr_script: &ScriptBuf,
    opreturn_script: &ScriptBuf,
) -> Transaction {
    let previous_output: OutputRef = ArbitraryGenerator::new().generate();

    let inputs = vec![TxIn {
        previous_output: *previous_output.outpoint(),
        script_sig: Default::default(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    }];

    // Construct the outputs
    let outputs = vec![
        TxOut {
            value: amt, // 10 BTC in satoshis
            script_pubkey: addr_script.clone(),
        },
        TxOut {
            value: Amount::ZERO, // Amount is zero for OP_RETURN
            script_pubkey: opreturn_script.clone(),
        },
    ];

    // Create the transaction
    Transaction {
        version: bitcoin::transaction::Version(2),
        lock_time: LockTime::ZERO,
        input: inputs,
        output: outputs,
    }
}

pub fn build_no_op_deposit_request_script(
    magic: Vec<u8>,
    dummy_block: Vec<u8>,
    dest_addr: Vec<u8>,
) -> ScriptBuf {
    let builder = script::Builder::new()
        .push_slice(PushBytesBuf::try_from(magic).unwrap())
        .push_slice(PushBytesBuf::try_from(dummy_block).unwrap())
        .push_slice(PushBytesBuf::try_from(dest_addr).unwrap());

    builder.into_script()
}

pub fn build_test_deposit_request_script(
    magic: Vec<u8>,
    dummy_block: Vec<u8>,
    dest_addr: Vec<u8>,
) -> ScriptBuf {
    let mut data = magic;
    data.extend(dummy_block);
    data.extend(dest_addr);
    let builder = script::Builder::new()
        .push_opcode(OP_RETURN)
        .push_slice(PushBytesBuf::try_from(data).unwrap());

    builder.into_script()
}

pub fn build_test_deposit_script(magic: Vec<u8>, dest_addr: Vec<u8>) -> ScriptBuf {
    let mut data = magic;
    data.extend(dest_addr);

    let builder = script::Builder::new()
        .push_opcode(OP_RETURN)
        .push_slice(PushBytesBuf::try_from(data).unwrap());

    builder.into_script()
}
