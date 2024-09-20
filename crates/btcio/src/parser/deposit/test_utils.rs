use std::str::FromStr;

use bitcoin::{absolute::LockTime, Address, Amount, ScriptBuf, Transaction, TxOut};

use super::DepositTxConfig;

pub fn generic_taproot_addr() -> Address {
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
        federation_address: generic_taproot_addr(),
    }
}

pub fn create_transaction_two_outpoints(amt: Amount, scr1: &ScriptBuf, scr2: &ScriptBuf ) -> Transaction {
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

