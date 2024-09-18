use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use alpen_express_db::stubs::bridge::StubTxStateDb;
use alpen_express_primitives::{
    bridge::{OperatorIdx, PublickeyTable, TxSigningData},
    l1::{BitcoinPsbt, BitcoinTxOut, OutputRef, SpendInfo},
};
use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    absolute::LockTime,
    opcodes::all::{OP_PUSHNUM_1, OP_RETURN},
    script::Builder,
    secp256k1::{rand, Keypair, PublicKey, SecretKey},
    taproot::{LeafVersion, TaprootBuilder, TaprootSpendInfo},
    transaction::Version,
    Address, Amount, Network, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use express_storage::ops::bridge::{BridgeTxStateOps, Context};
use musig2::{secp256k1::SECP256K1, KeyAggContext, SecNonce};
use rand::{Rng, RngCore};
use threadpool::ThreadPool;

/// Generate `count` (public key, private key) pairs as two separate [`Vec`].
pub fn generate_keypairs(count: usize) -> (Vec<PublicKey>, Vec<SecretKey>) {
    let mut secret_keys: Vec<SecretKey> = Vec::with_capacity(count);
    let mut pubkeys: Vec<PublicKey> = Vec::with_capacity(count);

    let mut pubkeys_set: HashSet<PublicKey> = HashSet::new();

    while pubkeys_set.len() != count {
        let sk = SecretKey::new(&mut rand::thread_rng());
        let keypair = Keypair::from_secret_key(SECP256K1, &sk);
        let pubkey = PublicKey::from_keypair(&keypair);

        if pubkeys_set.insert(pubkey) {
            secret_keys.push(sk);
            pubkeys.push(pubkey);
        }
    }

    (pubkeys, secret_keys)
}

pub fn generate_pubkey_table(table: &[PublicKey]) -> PublickeyTable {
    let pubkey_table = table
        .iter()
        .enumerate()
        .map(|(i, pk)| (i as OperatorIdx, *pk))
        .collect::<BTreeMap<OperatorIdx, PublicKey>>();

    PublickeyTable::from(pubkey_table)
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

/// Generate a mock unsigned tx with two scripts.
///
/// An unsigned tx has an empty script_sig/witness fields.
///
/// # Returns
///
/// A tuple containing:
///
/// 1) The created unsigned [`Transaction`].
/// 2) The [`TaprootSpendInfo`] to spend via a [`ScriptBuf`].
/// 3) The [`ScriptBuf`] that can be spent.
pub fn generate_mock_unsigned_tx(num_inputs: usize) -> (Transaction, TaprootSpendInfo, ScriptBuf) {
    // actually construct a valid taptree order to check PSBT finalization
    let (pks, _) = generate_keypairs(1);
    let internal_key = pks[0].x_only_public_key().0;

    let anyone_can_spend = Builder::new().push_opcode(OP_PUSHNUM_1).into_script();
    let none_can_spend = Builder::new().push_opcode(OP_RETURN).into_script();

    let taproot = TaprootBuilder::new()
        .add_leaf(1, anyone_can_spend.clone())
        .expect("should be able to add tapleaf")
        .add_leaf(1, none_can_spend.clone())
        .expect("should be able to add tapleaf");

    let spend_info = taproot
        .finalize(SECP256K1, internal_key)
        .expect("taproot build should work");
    let merkle_root = spend_info.merkle_root();

    let address = Address::p2tr(SECP256K1, internal_key, merkle_root, Network::Regtest);

    let random_bytes = vec![0u8; 1024];
    let mut unstructured = Unstructured::new(&random_bytes);
    let previous_output = *OutputRef::arbitrary(&mut unstructured)
        .expect("should be able to generate arbitrary output ref")
        .outpoint();

    let tx = Transaction {
        version: Version(2),
        lock_time: LockTime::ZERO,
        input: vec![
            TxIn {
                previous_output,
                script_sig: Default::default(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            };
            num_inputs // XXX: duplicating inputs like this should not have been allowed.
        ],
        output: vec![TxOut {
            value: Amount::from_sat(0), // so that we can add many inputs above
            script_pubkey: address.script_pubkey(), /* this is not accurate, we are actually
                                         * spending from this pubkey */
        }],
    };

    (tx, spend_info, anyone_can_spend)
}

/// Generate a mock spend info with arbitrary data.
pub fn generate_mock_spend_info() -> SpendInfo {
    let data = &[0u8; 1024];
    let mut unstructured = Unstructured::new(&data[..]);
    let spend_info: SpendInfo = SpendInfo::arbitrary(&mut unstructured).unwrap();

    spend_info
}

/// Create mock [`TxSigningData`]
pub fn generate_mock_tx_signing_data(num_inputs: usize) -> TxSigningData {
    // Create a minimal unsigned transaction
    let (unsigned_tx, spend_info, script_buf) = generate_mock_unsigned_tx(num_inputs);
    let prevouts = generate_mock_prevouts(num_inputs);

    let spend_info = SpendInfo {
        script_buf: script_buf.clone(),
        control_block: spend_info
            .control_block(&(script_buf, LeafVersion::TapScript))
            .expect("should be able to construct control block"),
    };

    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).expect("should be able to create psbt");
    for (i, input) in psbt.inputs.iter_mut().enumerate() {
        input.witness_utxo = Some(prevouts[i].clone());
    }

    let psbt = BitcoinPsbt::from(psbt);

    TxSigningData {
        psbt,
        spend_infos: vec![Some(spend_info); num_inputs],
    }
}

/// Create mock database ops to interact with the bridge tx state in a stubbed in-memory database.
pub fn generate_mock_tx_state_ops(num_threads: usize) -> BridgeTxStateOps {
    let storage = StubTxStateDb::default();
    let storage_ctx = Context::new(Arc::new(storage));

    let pool = ThreadPool::new(num_threads);

    storage_ctx.into_ops(pool)
}

/// Generate a MuSig2 sec nonce.
pub fn generate_sec_nonce(
    msg: &impl AsRef<[u8]>,
    pubkeys: impl IntoIterator<Item = PublicKey>,
    seckey: SecretKey,
    tweak: bool,
) -> SecNonce {
    let key_agg_ctx = KeyAggContext::new(pubkeys).expect("key agg context should be created");
    let key_agg_ctx = if tweak {
        key_agg_ctx
            .with_unspendable_taproot_tweak()
            .expect("should add unspendable taproot tweak")
    } else {
        key_agg_ctx
    };

    let aggregated_pubkey: PublicKey = key_agg_ctx.aggregated_pubkey();

    let mut nonce_seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce_seed);

    let sec_nonce = SecNonce::build(nonce_seed)
        .with_seckey(seckey)
        .with_message(msg)
        .with_aggregated_pubkey(aggregated_pubkey)
        .build();

    sec_nonce
}

/// Permute a list by successively swapping positions in the subslice 0..n, where n <= list.len().
/// This is used to generate random order for indices in a list (for example, list of pubkeys,
/// nonces, etc.)
pub fn permute<T: Clone>(list: &mut [T]) {
    let num_permutations = rand::thread_rng().gen_range(0..list.len());

    generate_permutation(list, num_permutations);
}

fn generate_permutation<T: Clone>(list: &mut [T], n: usize) {
    if n == 1 {
        return;
    }

    for i in 0..n {
        generate_permutation(list, n - 1);

        // Swap elements based on whether n is even or odd
        if n % 2 == 0 {
            list.swap(i, n - 1);
        } else {
            list.swap(0, n - 1);
        }
    }
}
