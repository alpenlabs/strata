//! Define the [`SignatureManager`] that is responsible for managing signatures for
//! [`Psbt`](bitcoin::Psbt)'s.

use std::{collections::BTreeMap, sync::Arc};

use alpen_express_db::entities::bridge_tx_state::BridgeTxState;
use alpen_express_primitives::{
    bridge::{
        Musig2PartialSig, Musig2PubNonce, OperatorIdx, OperatorPartialSig, PublickeyTable,
        TxSigningData,
    },
    l1::TaprootSpendPath,
};
use bitcoin::{
    hashes::Hash,
    key::{
        rand::{rngs::OsRng, RngCore},
        Keypair, TapTweak,
    },
    secp256k1::{schnorr::Signature, PublicKey, SecretKey, XOnlyPublicKey},
    sighash::SighashCache,
    witness::Witness,
    Transaction, Txid,
};
use express_storage::ops::bridge::BridgeTxStateOps;
use musig2::{
    aggregate_partial_signatures, secp256k1::SECP256K1, AggNonce, KeyAggContext, SecNonce,
};

use super::errors::{BridgeSigError, BridgeSigResult};
use crate::operations::{create_message_hash, sign_state_partial, verify_partial_sig};

/// Handle creation, collection and aggregation of signatures for a [`BridgeTxState`] with the help
/// of a persistence layer.
#[derive(Clone)]
pub struct SignatureManager {
    /// Abstraction over the persistence layer for the signatures.
    db_ops: Arc<BridgeTxStateOps>,

    /// This bridge client's keypair
    keypair: Keypair,

    /// This bridge client's Operator index.
    index: OperatorIdx,
}

impl std::fmt::Debug for SignatureManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "signature manager: {}", self.index)
    }
}

impl SignatureManager {
    /// Create a new [`SignatureManager`].
    pub fn new(db_ops: Arc<BridgeTxStateOps>, index: OperatorIdx, keypair: Keypair) -> Self {
        Self {
            db_ops,
            keypair,
            index,
        }
    }

    /// Adds a [`BridgeTxState`] to the [`SignatureManager`] replacing if already present for the
    /// computed [`Txid`].
    pub async fn add_tx_state(
        &self,
        tx_signing_data: TxSigningData,
        pubkey_table: PublickeyTable,
    ) -> BridgeSigResult<Txid> {
        let txid = tx_signing_data.psbt.compute_txid();

        // Catching this error will help avoid the tx from being replaced *after* the nonces have
        // already been shared. The flip side is that transactions cannot be replaced at all.
        if self.db_ops.get_tx_state_async(txid).await?.is_some() {
            return Err(BridgeSigError::DuplicateTransaction);
        }

        let key_agg_ctx = KeyAggContext::new(pubkey_table.0.values().copied())?;

        let keypath_spend_only = matches!(tx_signing_data.spend_path, TaprootSpendPath::Key);

        let key_agg_ctx = if keypath_spend_only {
            key_agg_ctx.with_unspendable_taproot_tweak()?
        } else {
            key_agg_ctx
        };

        let sec_nonce = self.generate_sec_nonce(&txid, &key_agg_ctx);
        let pub_nonce = sec_nonce.public_nonce();

        let mut tx_state = BridgeTxState::new(tx_signing_data, pubkey_table, sec_nonce.into())?;

        tx_state.add_nonce(&self.index, pub_nonce.into())?;

        self.db_ops.put_tx_state_async(txid, tx_state).await?;

        Ok(txid)
    }

    /// Get the state stored in the persistence layer for exposing state to an external consumer
    /// via the [`Txid`].
    ///
    /// # Errors
    ///
    /// If the state corresponding to the `txid` is not present in the database
    /// ([`BridgeSigError::TransactionNotFound`]). Or, if there is an error in the persistence layer
    /// itself ([`BridgeSigError::Storage`]).
    pub async fn get_tx_state(&self, txid: &Txid) -> BridgeSigResult<BridgeTxState> {
        let entry = self.db_ops.get_tx_state_async(*txid).await?;

        entry.ok_or(BridgeSigError::TransactionNotFound)
    }

    /// Generate a random secret nonce.
    ///
    /// Please refer to MuSig2 nonce generation section in
    /// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
    ///
    /// # Notes
    ///
    /// The entropy is pooled using the underlying operating system's
    /// cryptographic-safe pseudo-random number generator with [`OsRng`].
    fn generate_sec_nonce(&self, txid: &Txid, key_agg_ctx: &KeyAggContext) -> SecNonce {
        let aggregated_pubkey: PublicKey = key_agg_ctx.aggregated_pubkey();

        let mut nonce_seed = [0u8; 32];
        OsRng.fill_bytes(&mut nonce_seed);

        let seckey = SecretKey::from_keypair(&self.keypair);

        SecNonce::build(nonce_seed)
            .with_seckey(seckey)
            .with_message(txid.as_byte_array())
            .with_aggregated_pubkey(aggregated_pubkey)
            .build()
    }

    /// Get one's own public nonce for the given [`Txid`].
    ///
    /// Please refer to MuSig2 nonce generation section in
    /// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
    pub async fn get_own_nonce(&self, txid: &Txid) -> BridgeSigResult<Musig2PubNonce> {
        let tx_state = self.get_tx_state(txid).await?;

        let pubnonce = tx_state
            .collected_nonces()
            .get(&self.index)
            .expect("should always be present whenever a state is present");

        Ok(pubnonce.clone())
    }

    /// Add a nonce to the collection for given [`OperatorIdx`] and [`Txid`].
    ///
    /// The [`OperatorIdx`] may even be the same as `Self::index` in which case the nonce is
    /// updated. It is assumed that the upstream duty producer makes sure that the nonce only
    /// comes from a node authorized to produce that nonce.
    ///
    /// Please refer to MuSig2 nonce aggregation section in
    /// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
    /// # Returns
    ///
    /// A flag indicating whether adding the nonce completes the collection.
    pub async fn add_nonce(
        &self,
        txid: &Txid,
        operator_index: OperatorIdx,
        pub_nonce: &Musig2PubNonce,
    ) -> BridgeSigResult<bool> {
        let mut tx_state = self.get_tx_state(txid).await?;

        let is_complete = tx_state.add_nonce(&operator_index, pub_nonce.clone())?;
        self.db_ops.put_tx_state_async(*txid, tx_state).await?;

        Ok(is_complete)
    }

    /// Get the aggregated nonce from the list of collected nonces for the transaction
    /// corresponding to the given [`Txid`].
    ///
    /// Please refer to MuSig2 nonce aggregation section in
    /// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
    /// # Errors
    ///
    /// If not all nonces have been colllected yet.
    pub fn get_aggregated_nonce(&self, tx_state: &BridgeTxState) -> BridgeSigResult<AggNonce> {
        if !tx_state.has_all_nonces() {
            return Err(BridgeSigError::IncompleteNonces);
        }

        Ok(tx_state.ordered_nonces().into_iter().sum())
    }

    /// Add this bridge client's signature for the transaction.
    ///
    /// # Returns
    ///
    /// A flag indicating whether the [`alpen_express_primitives::l1::BitcoinPsbt`] being tracked in
    /// the [`BridgeTxState`] has become fully signed after adding the signature.
    pub async fn add_own_partial_sig(&self, txid: &Txid) -> BridgeSigResult<bool> {
        let mut tx_state = self.get_tx_state(txid).await?;

        let aggregated_nonce = self.get_aggregated_nonce(&tx_state)?;

        let unsigned_tx = tx_state.unsigned_tx().clone();

        let prevouts = tx_state.prevouts();

        let spend_info = tx_state.spend_path();
        let keypath_spend_only = matches!(spend_info, TaprootSpendPath::Key);

        let mut tx = unsigned_tx.clone();
        let mut sighash_cache = SighashCache::new(&mut tx);

        let message = create_message_hash(&mut sighash_cache, &prevouts, spend_info)?;

        let message = message.as_ref();

        let signature = sign_state_partial(
            tx_state.pubkeys(),
            tx_state.secnonce(),
            &self.keypair,
            &aggregated_nonce,
            message,
            keypath_spend_only,
        )?;

        let own_signature_info = OperatorPartialSig::new(signature.into(), self.index);
        verify_partial_sig(
            &tx_state,
            &own_signature_info,
            &aggregated_nonce,
            message,
            keypath_spend_only,
        )?;

        let is_fully_signed = tx_state.add_signature(own_signature_info)?;

        self.db_ops.put_tx_state_async(*txid, tx_state).await?;

        Ok(is_fully_signed)
    }

    /// Get this bridge client's own signature.
    pub async fn get_own_partial_sig(
        &self,
        txid: &Txid,
    ) -> BridgeSigResult<Option<Musig2PartialSig>> {
        let tx_state = self.get_tx_state(txid).await?;

        Ok(tx_state.collected_sigs().get(&self.index).copied())
    }

    /// Add a partial signature for a [`BridgeTxState`].
    ///
    /// The [`OperatorPartialSig::signer_index`] may even be the same as `Self::index` in which case
    /// the nonce is updated. It is assumed that the upstream duty producer makes sure that the
    /// nonce only comes from a node authorized to produce that nonce.
    ///
    /// Please refer to MuSig2 partial signature aggregation section in
    /// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
    ///
    /// # Returns
    ///
    /// A flag indicating whether the [`alpen_express_primitives::l1::BitcoinPsbt`] being tracked in
    /// the [`BridgeTxState`] has become fully signed after adding the signature.
    pub async fn add_partial_sig(
        &self,
        txid: &Txid,
        signature_info: OperatorPartialSig,
    ) -> BridgeSigResult<bool> {
        let mut tx_state = self.get_tx_state(txid).await?;

        let aggregated_nonce = self.get_aggregated_nonce(&tx_state)?;

        let mut unsigned_tx = tx_state.unsigned_tx().clone();
        let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

        let spend_info = tx_state.spend_path();
        let prevouts = tx_state.prevouts();

        let message = create_message_hash(&mut sighash_cache, &prevouts, spend_info)?;
        let message = message.as_ref();

        let keypath_spend_only = matches!(spend_info, TaprootSpendPath::Key);
        verify_partial_sig(
            &tx_state,
            &signature_info,
            &aggregated_nonce,
            message,
            keypath_spend_only,
        )?;

        tx_state.add_signature(signature_info)?;
        self.db_ops
            .put_tx_state_async(*txid, tx_state.clone())
            .await?;

        Ok(tx_state.is_fully_signed())
    }

    /// Retrieve the fully signed transaction for broadcasting.
    ///
    /// # Errors
    ///
    /// This method can error under the following conditions:
    ///
    /// 1. An error occurred while fetching the [`BridgeTxState`] from the database.
    /// 2. Not all required signatures have been collected.
    /// 3. The public keys could not be aggregated.
    /// 4. The sighash message to be signed could not be generated.
    /// 5. The collected signatures could not be aggregated.
    /// 6. The aggregated signature is not valid for the given message and aggregated pubkey.
    /// 7. A fully signed transaction could not be created from the [`bitcoin::Psbt`].
    /// 8. A tweak could not be applied to the key aggregation context for keypath spend.
    pub async fn get_fully_signed_transaction(&self, txid: &Txid) -> BridgeSigResult<Transaction> {
        let tx_state = self.get_tx_state(txid).await?;

        // this fails if not all nonces have been collected yet.
        let aggregated_nonce = self.get_aggregated_nonce(&tx_state)?;

        if !tx_state.is_fully_signed() {
            return Err(BridgeSigError::NotFullySigned);
        }

        let spend_info = tx_state.spend_path();
        let keypath_spend_only = matches!(spend_info, TaprootSpendPath::Key);

        let prevouts = &tx_state.prevouts();

        let key_agg_ctx = KeyAggContext::new(tx_state.pubkeys().0.values().clone().copied())?;
        let tweaked_key_agg_ctx = if keypath_spend_only {
            key_agg_ctx.with_unspendable_taproot_tweak()?
        } else {
            key_agg_ctx
        };

        let mut unsigned_tx = tx_state.unsigned_tx().clone();
        let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

        let mut psbt = tx_state.psbt().inner().clone();

        let partial_signatures = tx_state.ordered_sigs();

        let input = psbt.inputs.first_mut().expect("input must exist");
        let spend_info = spend_info.clone();

        // OPTIMIZE: this message is being created every time we sign a transaction in
        // `add_own_signature` *and* here as well. This is suboptimal computationally but the
        // alternative is to store it in the database for every input on every transaction which
        // is also wasteful (also involves creating a wrapper around `Message` to
        // implement serde*, borsh* and arbitrary traits).

        let message = create_message_hash(&mut sighash_cache, prevouts, &spend_info)?;

        let message_bytes = message.as_ref();

        let signature: Signature = aggregate_partial_signatures(
            &tweaked_key_agg_ctx,
            &aggregated_nonce,
            partial_signatures,
            message_bytes,
        )?;

        // Verify that the aggregated signature corresponds to the aggregated pubkey
        // in the P2TR address. In the keypath spend, the verification is done against the
        // tweaked pubkey and in the script path spend, it is done against the untweaked pubkey
        // (as the pubkey is present in the script being spent).
        let aggregated_pubkey: PublicKey = tweaked_key_agg_ctx.aggregated_pubkey_untweaked();
        let x_only_pubkey: XOnlyPublicKey = aggregated_pubkey.x_only_public_key().0;
        let x_only_pubkey: XOnlyPublicKey = if keypath_spend_only {
            x_only_pubkey.tap_tweak(SECP256K1, None).0.into()
        } else {
            x_only_pubkey
        };

        assert!(
            SECP256K1
                .verify_schnorr(&signature, &message, &x_only_pubkey)
                .is_ok(),
            "schnorr verification should succeed"
        );

        let mut witness = Witness::new();
        witness.push(signature.as_ref());

        // script spend path requires two additional witness elements --
        // the script and the control block.
        if let TaprootSpendPath::Script {
            script_buf,
            control_block,
        } = spend_info
        {
            witness.push(script_buf.to_bytes());
            witness.push(control_block.serialize());
        }

        // Finalize the psbt as per <https://github.com/rust-bitcoin/rust-bitcoin/blob/bitcoin-0.32.1/bitcoin/examples/taproot-psbt.rs#L315-L327>
        // NOTE: their ecdsa example states that we should use `miniscript` to finalize
        // PSBTs in production but they don't mention this for taproot.

        // Set final witness
        input.final_script_witness = Some(witness);

        // And clear all other fields as per the spec
        input.partial_sigs = BTreeMap::new();
        input.sighash_type = None;
        input.redeem_script = None;
        input.witness_script = None;
        input.bip32_derivation = BTreeMap::new();

        let signed_tx = psbt.extract_tx()?;

        Ok(signed_tx)
    }
}

#[cfg(test)]
mod tests {
    use std::{ops::Not, str::FromStr};

    use alpen_express_primitives::bridge::Musig2PartialSig;
    use alpen_test_utils::bridge::{
        generate_keypairs, generate_mock_tx_signing_data, generate_mock_tx_state_ops,
        generate_pubkey_table, generate_sec_nonce, permute,
    };
    use arbitrary::{Arbitrary, Unstructured};
    use bitcoin::{
        hashes::sha256,
        secp256k1::{PublicKey, SECP256K1},
    };
    use musig2::{secp256k1::Message, PubNonce};

    use super::*;

    #[tokio::test]
    async fn test_add_tx_state_keyspend() {
        test_add_tx_state(true).await;
    }

    #[tokio::test]
    async fn test_add_tx_state_scriptspend() {
        test_add_tx_state(false).await;
    }

    async fn test_add_tx_state(keypath_spend_only: bool) {
        let (_, secret_keys) = generate_keypairs(1);
        let self_index = 0;
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_keys[self_index as usize]);

        let signature_manager = generate_mock_manager(self_index, keypair);

        // Generate keypairs for the UTXO
        let (pubkeys, _) = generate_keypairs(3);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let tx_signing_data = generate_mock_tx_signing_data(keypath_spend_only);

        // Add TxState to the SignatureManager
        let result = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table.clone())
            .await;

        assert!(
            result.is_ok(),
            "should be able to add state to signature manager"
        );

        let txid = result.unwrap();

        let stored_tx_state = signature_manager.db_ops.get_tx_state_async(txid).await;
        assert!(stored_tx_state.is_ok(), "should retrieve saved state");

        let stored_tx_state = stored_tx_state.unwrap();
        assert!(stored_tx_state.is_some(), "state should exist in storage");

        let stored_tx_state = stored_tx_state.unwrap();

        let stored_pubkeys: Vec<PublicKey> = stored_tx_state.pubkeys().clone().into();
        assert_eq!(
            stored_pubkeys, pubkeys,
            "stored pubkeys and inserted pubkeys should be the same"
        );
        assert_eq!(
            stored_tx_state.psbt(),
            &tx_signing_data.psbt,
            "unsigned transaction in the storage and the one inserted must be the same"
        );

        let result = signature_manager
            .add_tx_state(tx_signing_data, pubkey_table)
            .await;
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::DuplicateTransaction)),
            "attempt to replace an existing tx state should fail with `DuplicateTransaction` error"
        );
    }

    #[test]
    fn test_generate_sec_nonce() {
        let (pks, sks) = generate_keypairs(5);
        let pubkey_table = generate_pubkey_table(&pks);

        let key_agg_ctx = KeyAggContext::new(pubkey_table.0.values().copied())
            .expect("should be able to create key aggregation context");

        let self_index = 0;
        let keypair = Keypair::from_secret_key(SECP256K1, &sks[0]);

        let sig_manager = generate_mock_manager(self_index, keypair);

        let keypath_spend_only = false;
        let txid = generate_mock_tx_signing_data(keypath_spend_only)
            .psbt
            .compute_txid();

        let result1 = sig_manager.generate_sec_nonce(&txid, &key_agg_ctx);
        let result2 = sig_manager.generate_sec_nonce(&txid, &key_agg_ctx);

        assert_ne!(
            result1, result2,
            "should generate different sec nonces even for the same context"
        );
    }

    #[tokio::test]
    async fn test_get_own_nonce_keyspend() {
        test_get_own_nonce(true).await;
    }

    #[tokio::test]
    async fn test_get_own_nonce_scriptspend() {
        test_get_own_nonce(false).await;
    }

    async fn test_get_own_nonce(keypath_spend_only: bool) {
        let own_index = 0;
        let num_operators = 2;
        assert!(
            num_operators.gt(&1) && num_operators.gt(&own_index),
            "num_operators should be set to greater than 1 and greater than self index"
        );

        let (pks, sks) = generate_keypairs(num_operators);
        let pubkey_table = generate_pubkey_table(&pks);

        let keypair = Keypair::from_secret_key(SECP256K1, &sks[own_index]);

        let tx_signing_data = generate_mock_tx_signing_data(keypath_spend_only);

        let sig_manager = generate_mock_manager(own_index as OperatorIdx, keypair);

        let txid = tx_signing_data.psbt.compute_txid();

        let own_pubnonce = sig_manager.get_own_nonce(&txid).await;
        assert!(
            own_pubnonce.is_err_and(|e| matches!(e, BridgeSigError::TransactionNotFound)),
            "should error with TransactionNotFound if the tx does not exist"
        );

        sig_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should be able to add tx state");

        let own_pubnonce = sig_manager.get_own_nonce(&txid).await;

        assert!(own_pubnonce.is_ok(), "should return own pubnonce");

        assert!(
            sig_manager
                .db_ops
                .get_tx_state_async(txid)
                .await
                .expect("storage should be accessible")
                .expect("state should be present")
                .collected_nonces()
                .get(&(own_index as OperatorIdx))
                .is_some_and(|n| *n == own_pubnonce.unwrap()),
            "stored nonce should match returned nonce"
        );
    }

    #[tokio::test]
    async fn test_get_aggregated_nonce_keyspend() {
        test_get_aggregated_nonce(true).await;
    }

    #[tokio::test]
    async fn test_get_aggregated_nonce_scriptspend() {
        test_get_aggregated_nonce(false).await;
    }

    async fn test_get_aggregated_nonce(keypath_spend_only: bool) {
        let own_index = 0;
        let num_operators = 2;
        assert!(
            num_operators.gt(&1) && num_operators.gt(&own_index),
            "num_operators should be set to greater than 1 and greater than self index"
        );

        let (pks, sks) = generate_keypairs(num_operators);
        let pubkey_table = generate_pubkey_table(&pks);

        let keypair = Keypair::from_secret_key(SECP256K1, &sks[own_index]);

        let tx_signing_data = generate_mock_tx_signing_data(false);

        let sig_manager = generate_mock_manager(own_index as OperatorIdx, keypair);

        let txid = tx_signing_data.psbt.compute_txid();

        sig_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should be able to add tx state");

        let state = sig_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("should be able to access stored state")
            .expect("state should be present");

        let result = sig_manager.get_aggregated_nonce(&state);
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::IncompleteNonces)),
            "should error with IncompleteNonces if not all nonces have been collected yet"
        );

        collect_nonces(
            &sig_manager,
            &txid,
            &pks,
            &sks,
            own_index,
            keypath_spend_only,
        )
        .await;

        let updated_state = sig_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("should be able to access state")
            .expect("state should be defined");

        let result = sig_manager.get_aggregated_nonce(&updated_state);
        assert!(
            result.is_ok(),
            "should produce aggregated nonce once all nonces have been aggregated but got: {}",
            result.err().unwrap()
        );
    }

    #[tokio::test]
    async fn test_add_own_partial_sig_scriptspend() {
        test_add_own_partial_sig(true).await;
    }

    #[tokio::test]
    async fn test_add_own_partial_sig_keyspend() {
        test_add_own_partial_sig(false).await;
    }

    async fn test_add_own_partial_sig(keypath_spend_only: bool) {
        let own_index = 2;
        let num_operators = 3;
        assert!(
            num_operators.gt(&1) && num_operators.gt(&own_index),
            "num_operators should be set to greater than 1 and greater than self and external index"
        );

        let (pks, sks) = generate_keypairs(num_operators);

        let keypair = Keypair::from_secret_key(SECP256K1, &sks[own_index]);

        let tx_signing_data = generate_mock_tx_signing_data(keypath_spend_only);

        let signature_manager = generate_mock_manager(own_index as OperatorIdx, keypair);

        let random_txid =
            Txid::from_str("4d3f5d9e4efc454d9e4e5f7b3e4c5f7d8e4f5d6e4c7d4f4e4d4d4d4e4d4d4d4d")
                .unwrap();

        let result = signature_manager.add_own_partial_sig(&random_txid).await;
        assert!(
            result.is_err(),
            "should error if the txid is not found in storage"
        );
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::TransactionNotFound)),
            "should error if the txid is not found in storage with `TransactionNotFound`"
        );

        let pubkey_table = generate_pubkey_table(&pks);
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should be able to add state");

        // Add the bridge client's own signature
        let result = signature_manager.add_own_partial_sig(&txid).await;
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::IncompleteNonces)),
            "should not be able to add own signature if not all nonces have been collected",
        );

        collect_nonces(
            &signature_manager,
            &txid,
            &pks,
            &sks,
            own_index,
            keypath_spend_only,
        )
        .await;

        let result = signature_manager.add_own_partial_sig(&txid).await;
        assert!(
            result.is_ok(),
            "should be able to add one's own signature once all nonces have been collected but got: {:?}", result.err().unwrap()
        );
        assert!(
            result.unwrap().not(),
            "only adding one's own signature should not make the psbt fully signed"
        );

        // Verify that the signature was added
        let stored_tx_state = signature_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("read state from db")
            .expect("state should be present");

        let collected_sigs = stored_tx_state.collected_sigs();

        // Ensure the signature is present in the first input
        assert!(
            collected_sigs.contains_key(&(own_index as OperatorIdx)),
            "own signature must be present in collected_sigs = {:?} at index: {}",
            collected_sigs,
            own_index
        );
    }

    #[tokio::test]
    async fn test_add_signature_keyspend() {
        test_add_signature(true).await;
    }

    #[tokio::test]
    async fn test_add_signature_scriptspend() {
        test_add_signature(false).await;
    }

    async fn test_add_signature(keypath_spend_only: bool) {
        let own_index = 1;
        let external_index = 0;

        let num_operators = 2;
        assert!(
            num_operators.eq(&2)
                && num_operators.gt(&own_index)
                && num_operators.gt(&external_index),
            "this test expects: num_operators == 2, > self_index, > external_index"
        );

        let (pks, sks) = generate_keypairs(num_operators);

        let self_keypair = Keypair::from_secret_key(SECP256K1, &sks[own_index]);

        let tx_signing_data = generate_mock_tx_signing_data(keypath_spend_only);

        let signature_manager = generate_mock_manager(own_index as OperatorIdx, self_keypair);

        let random_txid =
            Txid::from_str("4d3f5d9e4efc454d9e4e5f7b3e4c5f7d8e4f5d6e4c7d4f4e4d4d4d4e4d4d4d4d")
                .unwrap();

        let random_bytes = vec![0u8; 66];
        let mut unstructured = Unstructured::new(&random_bytes);
        let random_partial_sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate random partial sig");

        let invalid_sig_info =
            OperatorPartialSig::new(random_partial_sig, external_index as OperatorIdx);

        let result = signature_manager
            .add_partial_sig(&random_txid, invalid_sig_info)
            .await;
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::TransactionNotFound)),
            "error should be BridgeSigError::TransactionNotFound"
        );

        let pubkey_table = generate_pubkey_table(&pks);
        let keypath_spend_only = matches!(tx_signing_data.spend_path, TaprootSpendPath::Key); // sanity-check
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table.clone())
            .await
            .expect("should be able to add state");

        // Add the bridge client's own signature
        let result = signature_manager.add_own_partial_sig(&txid).await;
        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::IncompleteNonces)),
            "should not be able to add own signature if not all nonces have been collected",
        );

        let (_, sec_nonces) = collect_nonces(
            &signature_manager,
            &txid,
            &pks,
            &sks,
            own_index,
            keypath_spend_only,
        )
        .await;

        let tx_state = signature_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("storage should be accessible")
            .expect("state should be present");
        let agg_nonce = signature_manager
            .get_aggregated_nonce(&tx_state)
            .expect("should be able to get aggregated nonces");

        // Sign the transaction with an external key
        let mut unsigned_tx = tx_state.unsigned_tx().clone();

        let mut sighash_cache = SighashCache::new(&mut unsigned_tx);
        let message = create_message_hash(
            &mut sighash_cache,
            &tx_state.prevouts(),
            tx_state.spend_path(),
        )
        .expect("should be able to produce a message");

        let external_keypair = Keypair::from_secret_key(SECP256K1, &sks[external_index]);
        let external_signature = sign_state_partial(
            tx_state.pubkeys(),
            tx_state.secnonce(),
            &external_keypair,
            &agg_nonce,
            message.as_ref(),
            keypath_spend_only,
        )
        .unwrap();

        let external_signature_info =
            OperatorPartialSig::new(external_signature.into(), external_index as OperatorIdx);

        let result = signature_manager
            .add_partial_sig(&txid, external_signature_info)
            .await;
        assert!(
            result.is_err(),
            "one client's signature manager should not produce another's valid signature (different sec_nonce): {}",
            result.err().unwrap()
        );

        let external_signature = sign_state_partial(
            &pubkey_table,
            &sec_nonces[external_index].clone().into(),
            &external_keypair,
            &agg_nonce,
            message.as_ref(),
            keypath_spend_only,
        )
        .expect("should be able to produce partial sig");

        let external_signature_info =
            OperatorPartialSig::new(external_signature.into(), external_index as OperatorIdx);

        let result = signature_manager
            .add_partial_sig(&txid, external_signature_info)
            .await;

        assert!(
            result.is_ok(),
            "should be able to add valid partial sig to collection but got: {}",
            result.err().unwrap()
        );
        assert!(
            result.unwrap().not(),
            "should add the signature but not complete the collection"
        );

        // Verify that the signature was added
        let stored_tx_state = signature_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("should be able to load state")
            .expect("state should be present");

        assert!(
            stored_tx_state
                .collected_sigs()
                .get(&(external_index as OperatorIdx))
                .is_some_and(|sig| *sig.inner() == external_signature),
            "should have the external index at the right place"
        );

        let result = signature_manager.add_own_partial_sig(&txid).await;
        assert!(
            result.is_ok(),
            "should be able to add one's own partial sig but got: {}",
            result.err().unwrap()
        );

        assert!(result.unwrap(), "should complete the collection");

        let random_message = sha256::Hash::hash(b"random message").to_byte_array();
        let random_message = Message::from_digest_slice(&random_message).unwrap();
        let invalid_external_signature = sign_state_partial(
            tx_state.pubkeys(),
            tx_state.secnonce(),
            &external_keypair,
            &agg_nonce,
            random_message.as_ref(),
            keypath_spend_only,
        )
        .expect("should produce a signature");

        let invalid_external_signature_info = OperatorPartialSig::new(
            invalid_external_signature.into(),
            external_index as OperatorIdx,
        );

        let result = signature_manager
            .add_partial_sig(&txid, invalid_external_signature_info)
            .await;

        assert!(
            result.is_err_and(|e| matches!(e, BridgeSigError::InvalidSignature(_))),
            "should reject invalid signature"
        );
    }

    #[tokio::test]
    async fn test_get_fully_signed_transaction_keyspend() {
        test_get_fully_signed_transaction(true).await;
    }

    #[tokio::test]
    async fn test_get_fully_signed_transaction_scriptspend() {
        test_get_fully_signed_transaction(false).await;
    }

    async fn test_get_fully_signed_transaction(keypath_spend_only: bool) {
        // Generate keypairs for the UTXO
        let num_operators = 4;
        let (pubkeys, secret_keys) = generate_keypairs(num_operators);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let own_index = 2;
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_keys[own_index]);

        let signature_manager = generate_mock_manager(own_index as OperatorIdx, keypair);

        // Create a minimal unsigned transaction
        let tx_signing_data = generate_mock_tx_signing_data(keypath_spend_only);

        // Add TxState to the SignatureManager
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table.clone())
            .await
            .expect("should add state to storage");

        let (_pub_nonces, sec_nonces) = collect_nonces(
            &signature_manager,
            &txid,
            &pubkeys,
            &secret_keys,
            own_index,
            keypath_spend_only,
        )
        .await;

        let tx_state = signature_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("should be able to access storage")
            .expect("state should be available in the storage");

        let aggregated_nonce = signature_manager
            .get_aggregated_nonce(&tx_state)
            .expect("should be able to get aggregated nonces");

        // Add the bridge client's own signature
        let result = signature_manager.add_own_partial_sig(&txid).await;
        assert!(
            result.is_ok(),
            "should be able to add one's own partial sig but got error: {}",
            result.err().unwrap()
        );
        assert!(
            result.unwrap().not(),
            "adding 1 signature cannot complete the collection"
        );

        let prevouts = &tx_state.prevouts()[..];

        let mut operator_ids = (0..num_operators).collect::<Vec<usize>>();
        permute(&mut operator_ids);

        // Sign each input in the transaction with the other keys
        for signer_index in operator_ids {
            if signer_index == own_index {
                continue;
            }

            let mut unsigned_tx = tx_signing_data.psbt.inner().unsigned_tx.clone();
            let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

            let message = create_message_hash(&mut sighash_cache, prevouts, tx_state.spend_path());
            let message = message.expect("should be able to create message");

            let external_signature = sign_state_partial(
                &pubkey_table,
                &sec_nonces[signer_index].clone().into(),
                &Keypair::from_secret_key(SECP256K1, &secret_keys[signer_index]),
                &aggregated_nonce,
                message.as_ref(),
                keypath_spend_only,
            )
            .unwrap();

            let external_signature_info =
                OperatorPartialSig::new(external_signature.into(), signer_index as OperatorIdx);

            // Add the external signature
            let result = signature_manager
                .add_partial_sig(&txid, external_signature_info)
                .await;

            assert!(
                result.is_ok(),
                "should add external signature but got error: {:?} for {}",
                result.err().unwrap(),
                signer_index
            );

            // Verify that the signature has been added
            let stored_state = signature_manager
                .db_ops
                .get_tx_state_async(txid)
                .await
                .expect("should be able to access storage")
                .expect("should have tx state in the storage");

            assert!(stored_state
                .collected_sigs()
                .get(&(signer_index as OperatorIdx))
                .is_some_and(|sig| *sig.inner() == external_signature));
        }

        // Retrieve the fully signed transaction
        let signed_tx = signature_manager.get_fully_signed_transaction(&txid).await;
        assert!(
            signed_tx.is_ok(),
            "signed tx must be present but got error = {:?}",
            signed_tx.err()
        );

        let signed_tx = signed_tx.unwrap();

        // Verify that the signed transaction is not empty
        assert!(!signed_tx.input.is_empty());
        assert!(!signed_tx.output.is_empty());
    }

    fn generate_mock_manager(self_index: u32, keypair: Keypair) -> SignatureManager {
        let db_ops = generate_mock_tx_state_ops(1);

        SignatureManager::new(db_ops.into(), self_index, keypair)
    }

    async fn collect_nonces(
        sig_manager: &SignatureManager,
        txid: &Txid,
        pks: &[PublicKey],
        sks: &[SecretKey],
        own_index: usize, // should be `u32` but this leads to double conversion
        keypath_spend_only: bool,
    ) -> (Vec<PubNonce>, Vec<SecNonce>) {
        let mut pub_nonces = Vec::with_capacity(pks.len());
        let mut sec_nonces = Vec::with_capacity(sks.len());

        for (i, sk) in sks.iter().enumerate() {
            let sec_nonce = generate_sec_nonce(txid, pks.to_vec(), *sk, keypath_spend_only);
            let pub_nonce = sec_nonce.public_nonce();

            sec_nonces.push(sec_nonce);
            pub_nonces.push(pub_nonce.clone());

            // skip setting the nonce for this bridge client as that would already be set when a
            // state is added. It is fine to leave the above assignments (and even better) so as to
            // maintain the length of these lists.
            if i == own_index {
                continue;
            }
        }

        // jumble the order of operator ids which determines the order in which nonces are added to
        // the state
        let mut operator_ids = (0..pks.len()).collect::<Vec<usize>>();
        permute(&mut operator_ids);

        for operator_id in operator_ids {
            if operator_id == own_index {
                continue;
            }

            sig_manager
                .add_nonce(
                    txid,
                    operator_id as OperatorIdx,
                    &pub_nonces[operator_id].clone().into(),
                )
                .await
                .expect("should be able to add nonce");
        }

        (pub_nonces, sec_nonces)
    }
}
