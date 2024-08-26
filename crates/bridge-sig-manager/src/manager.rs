//! Define the [`SignatureManager`] that is responsible for managing signatures for
//! [`Psbt`](bitcoin::Psbt)'s.

use std::{collections::BTreeMap, sync::Arc};

use alpen_express_db::entities::bridge_tx_state::BridgeTxState;
use alpen_express_primitives::{
    bridge::{OperatorIdx, PublickeyTable, SignatureInfo, TxSigningData},
    l1::SpendInfo,
};
use bitcoin::{
    hashes::Hash,
    key::{Keypair, Secp256k1},
    secp256k1::{schnorr::Signature, All, Message},
    sighash::{self, Prevouts, SighashCache},
    taproot::LeafVersion,
    ScriptBuf, TapLeafHash, Transaction, TxOut, Txid, Witness,
};
use express_storage::ops::bridge::BridgeTxStateOps;

use super::errors::{BridgeSigError, BridgeSigResult};

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

    /// The secp engine used to sign messages and verify signatures against them.
    secp: Arc<Secp256k1<All>>,
}

impl std::fmt::Debug for SignatureManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "signature manager: {}", self.index)
    }
}

impl SignatureManager {
    /// Create a new [`SignatureManager`].
    pub fn new(
        db_ops: Arc<BridgeTxStateOps>,
        index: OperatorIdx,
        keypair: Keypair,
        secp: Arc<Secp256k1<All>>,
    ) -> Self {
        Self {
            db_ops,
            keypair,
            index,
            secp,
        }
    }

    /// Adds a [`BridgeTxState`] to the [`SignatureManager`] replacing if already present for the
    /// computed txid.
    pub async fn add_tx_state(
        &self,
        tx_signing_data: TxSigningData,
        pubkey_table: PublickeyTable,
    ) -> BridgeSigResult<Txid> {
        let tx_state = BridgeTxState::new(tx_signing_data, pubkey_table)?;
        let txid = tx_state.compute_txid();

        self.db_ops.upsert_tx_state_async(txid, tx_state).await?;

        Ok(txid)
    }

    /// Add this bridge client's signature for the transaction.
    ///
    /// # Returns
    ///
    /// A flag indicating whether the [`alpen_express_primitives::l1::BitcoinPsbt`] being tracked in
    /// the [`BridgeTxState`] has become fully signed after adding the signature.
    pub async fn add_own_signature(&self, txid: &Txid) -> BridgeSigResult<bool> {
        let tx_state = self.db_ops.get_tx_state_async(*txid).await?;

        if tx_state.is_none() {
            return Err(BridgeSigError::TransactionNotFound);
        }

        let mut tx_state = tx_state.unwrap();

        let mut unsigned_tx = tx_state.unsigned_tx().clone();
        let inputs = unsigned_tx.input.clone();

        let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

        let prevouts = &tx_state.prevouts();

        let mut is_fully_signed = false;
        for (input_index, _) in inputs.iter().enumerate() {
            let spend_infos = tx_state.spend_infos();
            let script = spend_infos[input_index].script_buf.clone();

            let signature = self.sign_tx(
                &mut sighash_cache,
                prevouts,
                input_index,
                &self.keypair,
                &script,
            )?;

            is_fully_signed = tx_state.add_signature(
                SignatureInfo::new(signature.into(), self.index),
                input_index,
            )?;

            // It may be that adding one's own signature causes the psbt to be completely signed.
            // This can happen if this bridge client receives the transaction information later than
            // other bridge clients.
            if is_fully_signed {
                break;
            }
        }

        self.db_ops
            .upsert_tx_state_async(*txid, tx_state.clone())
            .await?;

        Ok(is_fully_signed)
    }

    /// TODO: replace with wallet API instead.
    fn sign_tx(
        &self,
        sighash_cache: &mut SighashCache<&mut Transaction>,
        prevouts: &[TxOut],
        input_index: usize,
        keypair: &Keypair,
        script: &ScriptBuf,
    ) -> Result<Signature, BridgeSigError> {
        let sighash_type = sighash::TapSighashType::Default;
        let leaf_hash = TapLeafHash::from_script(script, LeafVersion::TapScript);
        let prevouts = Prevouts::All(prevouts);

        let sighash = sighash_cache.taproot_script_spend_signature_hash(
            input_index,
            &prevouts,
            leaf_hash,
            sighash_type,
        )?;

        let message = Message::from_digest_slice(sighash.as_byte_array()).expect("should be hash");
        let signature = self.secp.sign_schnorr(&message, keypair);

        Ok(signature)
    }

    /// Add a signature for a [`TxState`].
    ///
    /// # Returns
    ///
    /// A flag indicating whether the transaction is fully signed.
    pub async fn add_signature(
        &self,
        txid: &Txid,
        signature_info: SignatureInfo,
        input_index: usize,
    ) -> BridgeSigResult<bool> {
        let tx_state = self.db_ops.get_tx_state_async(*txid).await?;

        if tx_state.is_none() {
            return Err(BridgeSigError::TransactionNotFound);
        }

        let mut tx_state = tx_state.unwrap();

        self.verify_signature(&tx_state, &signature_info, input_index)?;

        tx_state.add_signature(signature_info, input_index)?;
        self.db_ops
            .upsert_tx_state_async(*txid, tx_state.clone())
            .await?;

        Ok(tx_state.is_fully_signed())
    }

    /// Verify signature info for a particular transaction for a particular input.
    pub fn verify_signature(
        &self,
        tx_state: &BridgeTxState,
        signature_info: &SignatureInfo,
        input_index: usize,
    ) -> BridgeSigResult<()> {
        let unsigned_tx = tx_state.unsigned_tx().clone();

        let mut sighash_cache = SighashCache::new(unsigned_tx);

        let prevouts = tx_state.prevouts();
        let prevouts = sighash::Prevouts::All(&prevouts);
        let script = tx_state.spend_infos()[input_index].script_buf.clone();

        let leaf_hash = TapLeafHash::from_script(&script, LeafVersion::TapScript);
        let sighash_type = sighash::TapSighashType::Default;

        let sighash = sighash_cache.taproot_script_spend_signature_hash(
            input_index,
            &prevouts,
            leaf_hash,
            sighash_type,
        )?;

        let msg = Message::from_digest_slice(sighash.as_byte_array()).expect("should be hash");

        let signer_index = signature_info.signer_index();
        let signer_pubkey = tx_state
            .pubkeys()
            .0
            .get(signer_index)
            .ok_or(BridgeSigError::UnauthorizedPubkey)?;

        let signature = *signature_info.signature();
        self.secp
            .verify_schnorr(
                &signature.into(),
                &msg,
                &signer_pubkey.x_only_public_key().0,
            )
            .map_err(|_e| BridgeSigError::InvalidSignature)?;

        Ok(())
    }

    /// Check if a [`Psbt`] is fully signed.
    pub async fn is_fully_signed(&self, txid: &Txid) -> BridgeSigResult<bool> {
        let tx_state = self.db_ops.get_tx_state_async(*txid).await?;

        if tx_state.is_none() {
            return Err(BridgeSigError::TransactionNotFound);
        }

        Ok(tx_state.unwrap().is_fully_signed())
    }

    /// Retrieve the fully signed transaction for broadcasting.
    pub async fn get_fully_signed_transaction(&self, txid: &Txid) -> BridgeSigResult<Transaction> {
        let tx_state = self.db_ops.get_tx_state_async(*txid).await?;

        if tx_state.is_none() {
            return Err(BridgeSigError::TransactionNotFound);
        }

        let tx_state = tx_state.unwrap();

        if !tx_state.is_fully_signed() {
            return Err(BridgeSigError::NotFullySigned);
        }

        let mut psbt = tx_state.psbt().inner().clone();

        psbt.inputs
            .iter_mut()
            .enumerate()
            .for_each(|(input_index, input)| {
                let mut witness = Witness::new();

                // Add signatures in reverse order to fulfill the spend script.
                for (index, _) in tx_state.pubkeys().0.values().rev().enumerate() {
                    let signature = tx_state
                        .collected_sigs()
                        .get(input_index)
                        .expect("input position should be defined")
                        .get(&(index as u32))
                        .expect("position of the key should be defined");

                    witness.push(Signature::from(*signature).as_ref());
                }

                let SpendInfo {
                    script_buf,
                    control_block,
                } = &tx_state.spend_infos()[input_index];
                witness.push(script_buf.to_bytes());
                witness.push(control_block.serialize());

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
            });

        let signed_tx = psbt.extract_tx()?;

        Ok(signed_tx)
    }
}

#[cfg(test)]
mod tests {
    use std::{ops::Not, str::FromStr, sync::Arc};

    use alpen_test_utils::bridge::{
        create_mock_tx_signing_data, create_mock_tx_state_ops, generate_keypairs,
        generate_pubkey_table,
    };
    use bitcoin::{hashes::sha256, secp256k1::PublicKey};

    use super::*;

    #[tokio::test]
    async fn test_add_tx_state() {
        let secp = Secp256k1::new();
        let (_, secret_keys) = generate_keypairs(&secp, 1);
        let self_index = 0;
        let keypair = Keypair::from_secret_key(&secp, &secret_keys[self_index as usize]);

        let signature_manager = create_mock_manager(self_index, keypair);

        // Generate keypairs for the UTXO
        let (pubkeys, _) = generate_keypairs(&secp, 3);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let tx_signing_data = create_mock_tx_signing_data(1);

        // Add TxState to the SignatureManager
        let result = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
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
            stored_tx_state.psbt().inner().unsigned_tx,
            tx_signing_data.unsigned_tx,
            "unsigned transaction in the storage and the one inserted must be the same"
        );
    }

    #[tokio::test]
    async fn test_add_own_signature() {
        let secp = Secp256k1::new();
        let (pubkeys, secret_keys) = generate_keypairs(&secp, 2);

        let self_index = 1;
        let keypair = Keypair::from_secret_key(&secp, &secret_keys[self_index]);

        let num_inputs = 1;
        let tx_signing_data = create_mock_tx_signing_data(num_inputs);

        let signature_manager = create_mock_manager(self_index as u32, keypair);

        let random_txid =
            Txid::from_str("4d3f5d9e4efc454d9e4e5f7b3e4c5f7d8e4f5d6e4c7d4f4e4d4d4d4e4d4d4d4d")
                .unwrap();
        let result = signature_manager.add_own_signature(&random_txid).await;
        assert!(
            result.is_err(),
            "should error if the txid is not found in storage"
        );
        assert!(
            matches!(result.err().unwrap(), BridgeSigError::TransactionNotFound),
            "error should be BridgeSigError::TransactionNotFound"
        );

        let pubkey_table = generate_pubkey_table(&pubkeys);
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should be able to add state");

        // Add the bridge client's own signature
        let result = signature_manager.add_own_signature(&txid).await;
        assert!(
            result.is_ok(),
            "should add own signature, error = {:?}",
            result.err()
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
            collected_sigs[0].contains_key(&(self_index as u32)),
            "own signature must be present in collected_sigs = {:?}",
            collected_sigs
        );
    }

    #[tokio::test]
    async fn test_add_signature() {
        let secp = Secp256k1::new();

        let (pubkeys, secret_keys) = generate_keypairs(&secp, 2);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let self_index = 1;
        let external_index = 0;

        let own_keypair = Keypair::from_secret_key(&secp, &secret_keys[self_index]);
        let signature_manager = create_mock_manager(self_index as u32, own_keypair);

        // Create a minimal unsigned transaction
        let num_inputs = 1;
        let tx_signing_data = create_mock_tx_signing_data(num_inputs);

        // Add TxState to the SignatureManager
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should be able to add state");

        // Sign the transaction with an external key (at external_index)
        let mut unsigned_tx = tx_signing_data.unsigned_tx.clone();

        let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

        let input_index = 0;
        let external_keypair = Keypair::from_secret_key(&secp, &secret_keys[external_index]);
        let external_signature = signature_manager
            .sign_tx(
                &mut sighash_cache,
                &tx_signing_data.prevouts[..],
                input_index,
                &external_keypair,
                &tx_signing_data.spend_infos[input_index].script_buf,
            )
            .unwrap();

        let external_signature_info =
            SignatureInfo::new(external_signature.into(), external_index as u32);

        // Add the external signature
        let result = signature_manager
            .add_signature(&txid, external_signature_info, input_index)
            .await;
        assert!(
            result.is_ok(),
            "should add external signature but got error: {:?}",
            result.err()
        );

        // Verify that the signature was added
        let stored_tx_state = signature_manager
            .db_ops
            .get_tx_state_async(txid)
            .await
            .expect("should be able to load state")
            .expect("state should be present");
        let collected_sigs = stored_tx_state.collected_sigs();

        // Ensure the external signature is present
        assert!(collected_sigs[input_index].contains_key(&(external_index as u32)));

        let random_message = sha256::Hash::hash(b"random message").to_byte_array();
        let random_message = Message::from_digest_slice(&random_message).unwrap();
        let invalid_external_signature = secp.sign_schnorr(
            &random_message,
            &Keypair::from_secret_key(&secp, &secret_keys[1]),
        );

        let invalid_external_signature_info =
            SignatureInfo::new(invalid_external_signature.into(), external_index as u32);
        let result = signature_manager
            .add_signature(&txid, invalid_external_signature_info, 0)
            .await;

        assert!(result.is_err(), "should reject invalid signature");
    }

    #[tokio::test]
    async fn test_is_fully_signed() {
        let secp = Secp256k1::new();
        let (pubkeys, secret_keys) = generate_keypairs(&secp, 3);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let self_index = 0;
        let keypair = Keypair::from_secret_key(&secp, &secret_keys[self_index]);

        let signature_manager = create_mock_manager(self_index as u32, keypair);

        let num_inputs = 2;
        let tx_signing_data = create_mock_tx_signing_data(num_inputs);

        // Add TxState to the SignatureManager
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should add tx state to storage");

        // Sign the transaction with the other keys
        for (i, secret_key) in secret_keys.iter().enumerate() {
            // skip own signature
            if i == self_index {
                continue;
            }

            let mut unsigned_tx = tx_signing_data.unsigned_tx.clone();
            let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

            for input_index in 0..num_inputs {
                let external_signature = signature_manager
                    .sign_tx(
                        &mut sighash_cache,
                        &tx_signing_data.prevouts[..],
                        input_index,
                        &Keypair::from_secret_key(&secp, secret_key),
                        &tx_signing_data.spend_infos[input_index].script_buf,
                    )
                    .unwrap();

                let external_signature_info =
                    SignatureInfo::new(external_signature.into(), i as u32);

                // Add the external signature
                let result = signature_manager
                    .add_signature(&txid, external_signature_info, input_index)
                    .await
                    .expect("should add valid external signature");

                assert!(result.not(), "transaction should not be fully signed");
            }
        }

        // Finally, add the bridge client's own signature
        let result = signature_manager.add_own_signature(&txid).await;
        assert!(
            result.is_ok(),
            "should add own signature but got error: {:?}",
            result.err().unwrap()
        );

        let is_fully_signed = signature_manager
            .is_fully_signed(&txid)
            .await
            .expect("should be able to access stored state");
        assert!(is_fully_signed, "stored transaction should be fully signed");

        assert_eq!(
            result.unwrap(),
            is_fully_signed,
            "should be fully signed when adding own signature at the last"
        );
    }

    #[tokio::test]
    async fn test_get_fully_signed_transaction() {
        let secp = Secp256k1::new();

        // Generate keypairs for the UTXO
        let (pubkeys, secret_keys) = generate_keypairs(&secp, 4);
        let pubkey_table = generate_pubkey_table(&pubkeys);

        let self_index = 2;
        let keypair = Keypair::from_secret_key(&secp, &secret_keys[self_index]);

        let signature_manager = create_mock_manager(self_index as u32, keypair);

        // Create a minimal unsigned transaction
        let num_inputs = 3;
        let tx_signing_data = create_mock_tx_signing_data(num_inputs);

        // Add TxState to the SignatureManager
        let txid = signature_manager
            .add_tx_state(tx_signing_data.clone(), pubkey_table)
            .await
            .expect("should add state to storage");

        // Add the bridge client's own signature
        let result = signature_manager.add_own_signature(&txid).await;
        assert!(result.is_ok());

        // Sign the transaction with the other keys
        for (i, secret_key) in secret_keys.iter().enumerate() {
            if i == self_index {
                continue;
            }

            let mut unsigned_tx = tx_signing_data.unsigned_tx.clone();
            let mut sighash_cache = SighashCache::new(&mut unsigned_tx);

            for input_index in 0..num_inputs {
                let external_signature = signature_manager
                    .sign_tx(
                        &mut sighash_cache,
                        &tx_signing_data.prevouts[..],
                        input_index,
                        &Keypair::from_secret_key(&secp, secret_key),
                        &tx_signing_data.spend_infos[input_index].script_buf,
                    )
                    .unwrap();

                let external_signature_info =
                    SignatureInfo::new(external_signature.into(), i as u32);

                // Add the external signature
                let result = signature_manager
                    .add_signature(&txid, external_signature_info, input_index)
                    .await;

                assert!(
                    result.is_ok(),
                    "should add external signature but got error: {:?}",
                    result.err().unwrap()
                );
            }
        }

        // Retrieve the fully signed transaction
        assert!(
            signature_manager
                .is_fully_signed(&txid)
                .await
                .expect("storage should be fine"),
            "txid should be fully signed"
        );

        let signed_tx = signature_manager.get_fully_signed_transaction(&txid).await;
        assert!(
            signed_tx.is_ok(),
            "signed tx must be present but got error = {}",
            signed_tx.err().unwrap()
        );

        let signed_tx = signed_tx.unwrap();

        // Verify that the signed transaction is not empty
        assert!(!signed_tx.input.is_empty());
        assert!(!signed_tx.output.is_empty());
    }

    fn create_mock_manager(self_index: u32, keypair: Keypair) -> SignatureManager {
        let db_ops = create_mock_tx_state_ops(1);

        SignatureManager::new(
            db_ops.into(),
            self_index,
            keypair,
            Arc::new(Secp256k1::new()),
        )
    }
}
