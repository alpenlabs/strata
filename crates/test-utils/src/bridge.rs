use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use alpen_express_db::stubs::bridge::StubTxStateStorage;
use alpen_express_primitives::{
    bridge::{OperatorIdx, PublickeyTable, TxSigningData},
    l1::{BitcoinTxOut, SpendInfo},
};
use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    absolute::LockTime,
    key::Secp256k1,
    secp256k1::{rand, All, Keypair, PublicKey, SecretKey},
    transaction::Version,
    Amount, Sequence, Transaction, TxIn, TxOut, Witness,
};
use express_storage::ops::bridge::{BridgeTxStateOps, Context};
use threadpool::ThreadPool;

/// Generate `count` (public key, private key) pairs as two separate [`Vec`].
pub fn generate_keypairs(secp: &Secp256k1<All>, count: usize) -> (Vec<PublicKey>, Vec<SecretKey>) {
    let mut secret_keys: Vec<SecretKey> = Vec::with_capacity(count);
    let mut pubkeys: Vec<PublicKey> = Vec::with_capacity(count);

    let mut pubkeys_set: HashSet<PublicKey> = HashSet::new();

    while pubkeys_set.len() != count {
        let sk = SecretKey::new(&mut rand::thread_rng());
        let keypair = Keypair::from_secret_key(secp, &sk);
        let pubkey = PublicKey::from_keypair(&keypair);

        if pubkeys_set.insert(pubkey) {
            secret_keys.push(sk);
            pubkeys.push(pubkey);
        }
    }

    (pubkeys, secret_keys)
}

pub fn generate_pubkey_table(table: &[PublicKey]) -> PublickeyTable {
    let mut pubkey_table: BTreeMap<OperatorIdx, PublicKey> = BTreeMap::new();
    for (idx, pk) in table.iter().enumerate() {
        pubkey_table.insert(idx as OperatorIdx, *pk);
    }

    PublickeyTable::try_from(pubkey_table).expect("indexes in an iter are always sorted")
}

/// Generate a list of arbitrary prevouts.
///
/// For now, each prevout is just a script with an `OP_TRUE` output.
pub fn generate_mock_prevouts(count: usize) -> Vec<TxOut> {
    let mut prevouts = Vec::with_capacity(count);

    for _ in 0..count {
        let data = &[0u8; 1024];
        let mut unstructured = Unstructured::new(&data[..]);
        let txout = BitcoinTxOut::arbitrary(&mut unstructured).unwrap();

        prevouts.push(TxOut::from(txout));
    }

    prevouts
}

/// Generate a mock unsigned tx.
///
/// An unsigned tx has an empty script_sig/witness fields.
pub fn generate_mock_unsigned_tx(num_inputs: usize) -> Transaction {
    Transaction {
        version: Version(2),
        lock_time: LockTime::ZERO,
        input: vec![
            TxIn {
                previous_output: Default::default(),
                script_sig: Default::default(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            };
            num_inputs // XXX: duplicating inputs like this should not have been allowed.
        ],
        output: vec![TxOut {
            value: Amount::from_sat(0), // so that we can add many inputs above
            script_pubkey: Default::default(),
        }],
    }
}

/// Generate a mock spend info with arbitrary data.
pub fn generate_mock_spend_info() -> SpendInfo {
    let data = &[0u8; 1024];
    let mut unstructured = Unstructured::new(&data[..]);
    let spend_info: SpendInfo = SpendInfo::arbitrary(&mut unstructured).unwrap();

    spend_info
}

/// Create mock [`TxSigningData`]
pub fn create_mock_tx_signing_data(num_inputs: usize) -> TxSigningData {
    // Create a minimal unsigned transaction
    let unsigned_tx = generate_mock_unsigned_tx(num_inputs);
    let prevouts = generate_mock_prevouts(num_inputs);
    let spend_infos = generate_mock_spend_info();

    TxSigningData {
        unsigned_tx,
        prevouts,
        spend_infos: vec![spend_infos; num_inputs],
    }
}

pub fn create_mock_tx_state_ops(num_threads: usize) -> BridgeTxStateOps {
    let storage = StubTxStateStorage::default();
    let storage_ctx = Context::new(Arc::new(storage));

    let pool = ThreadPool::new(num_threads);

    storage_ctx.into_ops(pool)
}
