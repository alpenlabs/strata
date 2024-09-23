use bitcoin::{
    absolute::LockTime, opcodes::all::OP_RETURN, script::{self, PushBytesBuf}, secp256k1::PublicKey, Amount, ScriptBuf, Transaction, TxOut
};

use super::DepositTxConfig;

pub fn generic_taproot_addr() -> PublicKey {
    let pubkey_bytes = hex::decode("02c72e8f3b6fd307c8edb32e8b53ed69c1f9269792088fc2fb756ce49cf3ad46a8").expect("Decoding failed");
    PublicKey::from_slice(&pubkey_bytes).expect("Invalid public key")
}

pub fn get_deposit_tx_config() -> DepositTxConfig {
    DepositTxConfig {
        magic_bytes: "expresssss".to_string().as_bytes().to_vec(),
        address_length: 20,
        deposit_quantity: 1000,
        federation_address: generic_taproot_addr(),
    }
}

pub fn create_transaction_two_outpoints(
    amt: Amount,
    scr1: &ScriptBuf,
    scr2: &ScriptBuf,
) -> Transaction {
    // Construct the outputs
    let outputs = vec![
        TxOut {
            value: amt, // 10 BTC in satoshis
            // script_pubkey: taproot_addr().script_pubkey(),
            script_pubkey: scr1.clone(),
        },
        TxOut {
            value: Amount::ZERO, // Amount is zero for OP_RETURN
            script_pubkey: scr2.clone(),
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
    let builder = script::Builder::new()
        .push_opcode(OP_RETURN)
        .push_slice(PushBytesBuf::try_from(magic).unwrap())
        .push_slice(PushBytesBuf::try_from(dummy_block).unwrap())
        .push_slice(PushBytesBuf::try_from(dest_addr).unwrap());

    builder.into_script()
}

pub fn build_test_deposit_script(magic: Vec<u8>, dest_addr: Vec<u8>) -> ScriptBuf {
    let builder = script::Builder::new()
        .push_opcode(OP_RETURN)
        .push_slice(PushBytesBuf::try_from(magic).unwrap())
        .push_slice(PushBytesBuf::try_from(dest_addr).unwrap());

    builder.into_script()
}
