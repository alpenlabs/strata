//! Provides wallet-like functionalities for creating nonces and signatures.

use bitcoin::{
    hashes::Hash,
    secp256k1::{Keypair, Message, SecretKey},
    sighash::{self, Prevouts, SighashCache},
    taproot::LeafVersion,
    ScriptBuf, TapLeafHash, Transaction, TxOut,
};
use musig2::{sign_partial, verify_partial, AggNonce, KeyAggContext, PartialSignature};
use strata_db::entities::bridge_tx_state::BridgeTxState;
use strata_primitives::bridge::{Musig2SecNonce, OperatorPartialSig, PublickeyTable};

use crate::errors::{BridgeSigError, BridgeSigResult};

/// Generate a sighash message for a taproot `script` spending path at the `input_index` of
/// all `prevouts`.
pub fn create_script_spend_hash(
    sighash_cache: &mut SighashCache<&mut Transaction>,
    input_index: usize,
    script: &ScriptBuf,
    prevouts: &[TxOut],
) -> BridgeSigResult<Message> {
    let sighash_type = sighash::TapSighashType::Default;
    let leaf_hash = TapLeafHash::from_script(script, LeafVersion::TapScript);
    let prevouts = Prevouts::All(prevouts);

    let sighash = sighash_cache.taproot_script_spend_signature_hash(
        input_index,
        &prevouts,
        leaf_hash,
        sighash_type,
    )?;

    let message =
        Message::from_digest_slice(sighash.as_byte_array()).expect("TapSigHash is a hash");

    Ok(message)
}

/// Generate a partial MuSig2 signature for the given message and nonce values.
///
/// Please refer to MuSig2 signing section in
/// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
// TODO: replace this with a call to a wallet.
pub fn sign_state_partial(
    pubkey_table: &PublickeyTable,
    secnonce: &Musig2SecNonce,
    keypair: &Keypair,
    aggregated_nonce: &AggNonce,
    message: impl AsRef<[u8]>,
) -> BridgeSigResult<PartialSignature> {
    let pubkeys = pubkey_table.0.clone();
    let pubkeys = pubkeys.values();

    let secnonce = secnonce.inner().clone();

    let key_agg_ctx = KeyAggContext::new(pubkeys.copied())?;
    let seckey = SecretKey::from_keypair(keypair);

    let partial_sig: PartialSignature = sign_partial(
        &key_agg_ctx,
        seckey,
        secnonce,
        aggregated_nonce,
        message.as_ref(),
    )?;

    Ok(partial_sig)
}

/// Verify that a partial MuSig2 signature is correct.
pub fn verify_partial_sig(
    tx_state: &BridgeTxState,
    signature_info: &OperatorPartialSig,
    aggregated_nonce: &AggNonce,
    message: impl AsRef<[u8]>,
) -> BridgeSigResult<()> {
    let signer_index = signature_info.signer_index();

    let individual_pubkey = tx_state.pubkeys().0.get(signer_index);

    if individual_pubkey.is_none() {
        return Err(BridgeSigError::UnauthorizedPubkey);
    }

    let individual_pubkey = individual_pubkey.expect("should be some value");

    let pubkeys = tx_state.pubkeys().0.values().copied();
    let key_agg_ctx = KeyAggContext::new(pubkeys)?;

    let individual_pubnonce = tx_state
        .collected_nonces()
        .get(signer_index)
        .ok_or(BridgeSigError::PubNonceNotFound)?;

    let partial_signature = *signature_info.signature().inner();

    Ok(verify_partial(
        &key_agg_ctx,
        partial_signature,
        aggregated_nonce,
        *individual_pubkey,
        individual_pubnonce.inner(),
        message,
    )?)
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use bitcoin::{
        key::rand::{self, RngCore},
        secp256k1::{PublicKey, SECP256K1},
        Amount,
    };
    use musig2::{PubNonce, SecNonce};
    use strata_primitives::bridge::{Musig2PartialSig, OperatorIdx};
    use test_utils::bridge::{
        generate_keypairs, generate_mock_tx_signing_data, generate_mock_unsigned_tx,
        generate_pubkey_table, permute,
    };

    use super::*;

    #[test]
    fn test_create_script_spend_hash() {
        let num_inputs = 1;
        let (mut tx, _, _) = generate_mock_unsigned_tx(num_inputs);
        let mut sighash_cache = SighashCache::new(&mut tx);

        // Create dummy input values
        let input_index = 0;
        let script = ScriptBuf::new();
        let prevouts = vec![TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new(),
        }];

        let result = create_script_spend_hash(&mut sighash_cache, input_index, &script, &prevouts);

        assert!(
            result.is_ok(),
            "Failed to create script spend hash due to: {}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_generate_and_verify_partial_sig() {
        // Step 0: Setup

        let num_operators = 3;
        let own_index = 1;
        let (sks, aggregated_nonce, tx_state) = setup(num_operators, own_index);
        let txid = tx_state.unsigned_tx().compute_txid();

        // Step 1: Generate a partial signature

        let keypair = Keypair::from_secret_key(SECP256K1, &sks[own_index]);
        let partial_sig_result = sign_state_partial(
            tx_state.pubkeys(),
            tx_state.secnonce(),
            &keypair,
            &aggregated_nonce,
            txid.as_byte_array(),
        );

        assert!(
            partial_sig_result.is_ok(),
            "failed to generate partial signature: {}",
            partial_sig_result.err().unwrap()
        );

        let partial_sig = partial_sig_result.unwrap();

        // Step 2: Verify the partial signature

        let signature_info = OperatorPartialSig::new(partial_sig.into(), own_index as OperatorIdx);

        let verify_result = verify_partial_sig(
            &tx_state,
            &signature_info,
            &aggregated_nonce,
            txid.as_byte_array(),
        );

        assert!(
            verify_result.is_ok(),
            "failed to verify partial signature: {}",
            verify_result.err().unwrap()
        );

        // Step 3: Check error cases

        // Test 3.1: Right signature wrong position
        let signature_info = OperatorPartialSig::new(
            partial_sig.into(),
            ((own_index + 1) % num_operators).try_into().unwrap(),
        );

        let verify_result = verify_partial_sig(
            &tx_state,
            &signature_info,
            &aggregated_nonce,
            txid.as_byte_array(),
        );

        assert!(
            verify_result.is_err_and(|e| matches!(e, BridgeSigError::InvalidSignature(_))),
            "signature verification should fail with the error InvalidSignature",
        );

        // Test 3.2: Wrong signature
        let data = vec![0u8; 1024];
        let mut unstructured = Unstructured::new(&data);
        let random_partial_sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate an arbitrary partial sig");
        let signature_info = OperatorPartialSig::new(
            random_partial_sig,
            ((own_index + 1) % num_operators).try_into().unwrap(),
        );

        let verify_result = verify_partial_sig(
            &tx_state,
            &signature_info,
            &aggregated_nonce,
            txid.as_byte_array(),
        );

        assert!(
            verify_result.is_err_and(|e| matches!(e, BridgeSigError::InvalidSignature(_))),
            "signature verification should fail with the error InvalidSignature",
        );

        // Test 3.3: Wrong index
        let signature_info =
            OperatorPartialSig::new(partial_sig.into(), (num_operators + 1).try_into().unwrap());

        let verify_result = verify_partial_sig(
            &tx_state,
            &signature_info,
            &aggregated_nonce,
            txid.as_byte_array(),
        );

        assert!(
            verify_result.is_err_and(|e| matches!(e, BridgeSigError::UnauthorizedPubkey)),
            "signature verification should fail with the error UnauthorizedPubkey",
        );
    }

    fn setup(num_operators: usize, own_index: usize) -> (Vec<SecretKey>, AggNonce, BridgeTxState) {
        assert!(own_index.lt(&num_operators), "invalid own index set");

        let (pks, sks) = generate_keypairs(SECP256K1, num_operators);
        let pubkey_table = generate_pubkey_table(&pks);

        let num_inputs = 1;
        let tx_output = generate_mock_tx_signing_data(num_inputs);
        let txid = tx_output.psbt.inner().unsigned_tx.compute_txid();

        let key_agg_ctx =
            KeyAggContext::new(pks.clone()).expect("generation of key agg context should work");
        let aggregated_pubkey: PublicKey = key_agg_ctx.aggregated_pubkey();

        let mut pub_nonces: Vec<PubNonce> = Vec::with_capacity(pks.len());
        let mut sec_nonces: Vec<SecNonce> = Vec::with_capacity(sks.len());

        // check in reverse (or some permutation)
        for sk in sks.iter() {
            let mut nonce_seed = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut nonce_seed);

            let sec_nonce = SecNonce::build(nonce_seed)
                .with_seckey(*sk)
                .with_message(txid.as_byte_array())
                .with_aggregated_pubkey(aggregated_pubkey)
                .build();
            let pub_nonce = sec_nonce.public_nonce();

            sec_nonces.push(sec_nonce);
            pub_nonces.push(pub_nonce);
        }

        let aggregated_nonce = pub_nonces.iter().sum();

        let mut tx_state = BridgeTxState::new(
            tx_output,
            pubkey_table,
            sec_nonces[own_index].clone().into(),
        )
        .expect("Failed to create TxState");

        let mut nonces_complete = false;
        let mut permuted_pub_nonces = pub_nonces.clone();
        permute(&mut permuted_pub_nonces);

        for (i, pub_nonce) in pub_nonces.iter().enumerate() {
            nonces_complete = tx_state
                .add_nonce(&(i as OperatorIdx), pub_nonce.clone().into())
                .expect("should be able to add nonce");
        }

        assert!(
            nonces_complete,
            "adding the final nonce should complete the collection"
        );

        (sks, aggregated_nonce, tx_state)
    }
}
