use std::str::FromStr;

use bitcoin::{
    absolute::LockTime,
    opcodes::all::OP_RETURN,
    script::{self, PushBytesBuf},
    secp256k1::PublicKey,
    Address, Amount, ScriptBuf, Transaction, TxOut,
};

use super::DepositTxConfig;

pub fn generic_taproot_addr() -> Address {
    Address::from_str("bcrt1pnmrmugapastum8ztvgwcn8hvq2avmcwh2j4ssru7rtyygkpqq98q4wyd6s")
        .unwrap()
        .require_network(bitcoin::Network::Regtest)
        .unwrap()
}

pub fn generic_pubkey() -> PublicKey {
    let pubkey_bytes =
        hex::decode("041a787b3a3d4446103afc2886a7a34206cbbe8d4ddea93922c49df094bdb47ff4c25efe9cf6f407447f3c0a006263977c8dc6da04450ddf982463c34825627287")
            .expect("Decoding failed");
    PublicKey::from_slice(&pubkey_bytes).expect("Invalid public key")
}

pub fn get_deposit_tx_config() -> DepositTxConfig {
    DepositTxConfig {
        magic_bytes: "expresssss".to_string().as_bytes().to_vec(),
        address_length: 20,
        deposit_quantity: 1_000_000_000,
        federation_address: generic_pubkey(),
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
