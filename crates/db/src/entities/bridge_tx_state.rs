//! Defines the [`TxState`] type that tracks the state of signature collection for a particular
//! [`Psbt`].

use std::{collections::HashMap, ops::Not};

use alpen_express_primitives::{
    bridge::{
        Musig2PartialSig, Musig2PubNonce, Musig2SecNonce, OperatorIdx, PublickeyTable,
        SignatureInfo, TxSigningData,
    },
    l1::{BitcoinPsbt, BitcoinTxOut, SpendInfo},
};
use arbitrary::Arbitrary;
use bitcoin::{Psbt, Transaction, TxOut, Txid};
use borsh::{BorshDeserialize, BorshSerialize};
use musig2::{PartialSignature, PubNonce};

use super::errors::{BridgeTxStateError, EntityError, EntityResult};

/// The state a transaction is in with respect to the number of signatures that have been collected
/// from the bridge federation signatories.
#[derive(Debug, Clone, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct BridgeTxState {
    /// The partially signed bitcoin transaction that this state tracks.
    psbt: BitcoinPsbt,

    /// The prevouts of the unsigned transaction required for taproot signing.
    prevouts: Vec<BitcoinTxOut>,

    /// The witness elements required to spend a taproot output.
    spend_infos: Vec<SpendInfo>,

    /// The table of pubkeys that is used to lock the UTXO present as an input in the psbt.
    /// This table maps the `OperatorIdx` to their corresponding pubkeys.
    pubkey_table: PublickeyTable,

    /// The number of required signatures (same as the length of [`Self::pubkeys`])
    required_signatures: usize,

    /// The private nonce unique to the transaction being tracked by this state.
    // FIXME: storing the secret nonce in the db is not a good practice but since it needs to be
    // generated uniquely per transaction that is to be signed, we store it here.
    // For more on nonce security, see [this](https://docs.rs/musig2/latest/musig2/#security).
    secnonce: Musig2SecNonce,

    /// The (public) nonces shared for the particular [`Psbt`] that this state tracks under MuSig2.
    /// **NOTE**: The keys are not sorted. To get the ordered nonces, use the corresponding
    /// `ordered_*` getter.
    collected_nonces: HashMap<OperatorIdx, Musig2PubNonce>,

    /// The table of signatures collected so far per operator and per input in the [`Self::psbt`].
    /// **NOTE**: The keys are not sorted. To get the ordered nonces, use the corresponding
    /// `ordered_*` getter.
    collected_sigs: Vec<HashMap<OperatorIdx, Musig2PartialSig>>,
}

impl BridgeTxState {
    /// Create a new [`TxState`] for the given [`Psbt`] and list of [`PublicKey`].
    pub fn new(
        tx_signing_data: TxSigningData,
        pubkey_table: PublickeyTable,
        sec_nonce: Musig2SecNonce,
    ) -> EntityResult<Self> {
        let num_keys = pubkey_table.0.len();
        let mut psbt = Psbt::from_unsigned_tx(tx_signing_data.unsigned_tx)
            .map_err(BridgeTxStateError::from)?;
        let num_inputs = psbt.inputs.len();

        let prevouts: Vec<TxOut> = tx_signing_data.prevouts;

        for (i, each_psbt) in psbt.inputs.iter_mut().enumerate() {
            each_psbt.witness_utxo = Some(prevouts[i].clone());
        }

        let collected_sigs = vec![HashMap::new(); num_inputs];
        let prevouts = prevouts
            .into_iter()
            .map(BitcoinTxOut::from)
            .collect::<Vec<BitcoinTxOut>>();

        let collected_nonces: HashMap<OperatorIdx, Musig2PubNonce> = HashMap::new();

        Ok(Self {
            psbt: psbt.into(),
            prevouts,
            spend_infos: tx_signing_data.spend_infos,
            required_signatures: num_keys,
            pubkey_table,
            secnonce: sec_nonce,
            collected_nonces,
            collected_sigs,
        })
    }

    /// Get the [`Psbt`] that this state is associated with.
    pub fn psbt(&self) -> &BitcoinPsbt {
        &self.psbt
    }

    /// Get the spend info associated with each input in the PSBT.
    pub fn spend_infos(&self) -> &[SpendInfo] {
        &self.spend_infos[..]
    }

    /// Get the relevant previous outputs of the transaction that this state tracks.
    pub fn prevouts(&self) -> Vec<TxOut> {
        self.prevouts.iter().cloned().map(TxOut::from).collect()
    }

    /// Get the number of required signatures for the [`Psbt`] to be considered fully signed.
    pub fn required_signatures(&self) -> &usize {
        &self.required_signatures
    }

    /// Get the private nonce for the transaction being tracked in this state.
    pub fn secnonce(&self) -> &Musig2SecNonce {
        &self.secnonce
    }

    /// Get the [`PublickeyTable`] that maps [`OperatorIdx`] to the corresponding `PublicKey`
    /// correspondng to the multisig.
    pub fn pubkeys(&self) -> &PublickeyTable {
        &self.pubkey_table
    }

    /// Get the unsigned transaction from the [`Psbt`].
    pub fn unsigned_tx(&self) -> &Transaction {
        &self.psbt().inner().unsigned_tx
    }

    /// Compute the Transaction ID for the PSBT that this state tracks.
    pub fn compute_txid(&self) -> Txid {
        self.psbt.inner().unsigned_tx.compute_txid()
    }

    /// Get the map of collected nonces.
    pub fn collected_nonces(&self) -> &HashMap<OperatorIdx, Musig2PubNonce> {
        &self.collected_nonces
    }

    /// Get table of signatures collected so far per input in the transaction.
    pub fn collected_sigs(&self) -> &[HashMap<OperatorIdx, Musig2PartialSig>] {
        &self.collected_sigs
    }

    /// Get the ordered list of nonces collected so far.
    // NOTE: As accessing the list of nonces is usually done to `sum` them up, it's convenient to
    // return an iterator over the inner `PubNonce` type.
    pub fn ordered_nonces(&self) -> impl IntoIterator<Item = PubNonce> {
        let mut ordered_nonces: Vec<PubNonce> =
            Vec::with_capacity(self.collected_nonces.keys().len());

        for operator_idx in self.collected_nonces.keys() {
            if let Some(nonce) = self.collected_nonces.get(operator_idx) {
                ordered_nonces.push(nonce.inner().clone());
            }
        }

        // TODO: replace with yield when we get generators
        ordered_nonces.into_iter()
    }

    /// Check if all the nonces have been received.
    ///
    /// # Returns
    ///
    /// The aggregated nonce if all the required nonces have been collected, otherwise `None`.
    pub fn has_all_nonces(&self) -> bool {
        // Since we only add valid nonces (by checking the pubkey table), just checking the length
        // should be sufficient.
        self.collected_nonces.keys().len() == self.required_signatures
    }

    /// Add a nonce to the collected nonces.
    ///
    /// # Returns
    ///
    /// A flag indicating whether adding this nonce completes the collection.
    pub fn add_nonce(
        &mut self,
        operator_index: &OperatorIdx,
        nonce: Musig2PubNonce,
    ) -> EntityResult<bool> {
        if self.pubkey_table.0.contains_key(operator_index).not() {
            return Err(EntityError::BridgeOpUnauthorized);
        }

        self.collected_nonces.insert(*operator_index, nonce);

        Ok(self.has_all_nonces())
    }

    /// Check if all the required signatures have been collected for the [`Psbt`].
    pub fn is_fully_signed(&self) -> bool {
        // for each input, check all signatures have been collected
        // each signature is added only if the signer is part of the `pubkey_table`,
        // so checking the total number of signatures so far suffices.
        self.collected_sigs
            .iter()
            .all(|input| input.keys().len() == self.required_signatures)
    }

    /// Add a signature to the collection. If the signature corresponding to a particular pubkey has
    /// already been added, it is updated.
    ///
    /// **Note**: This being a database-related operation, no validation is performed on the
    /// provided signature as that requires access to a signing module. The only validation that
    /// this method performs is that the signature comes from an [`OperatorIdx`] that is part of
    /// the [`Self::pubkey_table`]. It is assumed that all necessary validation has already been
    /// performed at the callsite.
    ///
    /// # Returns
    ///
    /// A boolean flag indicating whether the added signature completes the set of required
    /// signatures.
    ///
    /// # Errors
    ///
    /// If the [`SignatureInfo::signer_index`] is not a part of the required signatories or the
    /// `input_index` is not part of the [`Psbt`].
    pub fn add_signature(
        &mut self,
        signature_info: SignatureInfo,
        input_index: usize,
    ) -> EntityResult<bool> {
        if self.psbt().inner().inputs.get(input_index).is_none() {
            let txid = self.compute_txid();
            return Err(BridgeTxStateError::TxinIdxOutOfBounds(txid, input_index))?;
        }

        // Some extra validation (should also be done by the rollup node)
        // Check if the signer is authorized i.e., they are part of the federation.
        let signer_index = signature_info.signer_index();

        if self.pubkey_table.0.contains_key(signer_index).not() {
            return Err(BridgeTxStateError::Unauthorized)?;
        }

        self.collected_sigs[input_index].insert(*signer_index, *signature_info.signature());

        Ok(self.is_fully_signed())
    }

    /// Get the ordered signatures per input collected so far.
    pub fn ordered_sigs(&self) -> Vec<Vec<PartialSignature>> {
        let num_inputs = self.collected_sigs.len();
        let mut ordered_sigs = vec![Vec::with_capacity(self.required_signatures); num_inputs];

        for (input_index, sigs) in self.collected_sigs.iter().enumerate() {
            for operator_idx in self.pubkey_table.0.keys() {
                if let Some(sig) = sigs.get(operator_idx) {
                    ordered_sigs[input_index].push(*sig.inner());
                }
            }
        }

        ordered_sigs
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::bridge::{
        generate_keypairs, generate_mock_tx_signing_data, generate_pubkey_table, generate_sec_nonce,
    };
    use arbitrary::Unstructured;
    use bitcoin::{key::Secp256k1, secp256k1::All};

    use super::*;
    use crate::entities::errors::EntityError;

    #[test]
    fn test_has_all_nonces() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 2;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        assert!(
            !tx_state.has_all_nonces(),
            "expected: false since no nonces have been collected but got: true"
        );

        let data = vec![0u8; 1024];
        let mut unstructured = Unstructured::new(&data[..]);

        for i in 0..num_operators {
            let random_nonce = Musig2PubNonce::arbitrary(&mut unstructured)
                .expect("should produce random pubnonce");

            tx_state
                .add_nonce(&(i as u32), random_nonce)
                .expect("should be able to add nonce");
        }

        assert!(
            tx_state.has_all_nonces(),
            "expected: true but got: false for the collected nonces: {:?}",
            tx_state.collected_nonces()
        );
    }

    #[test]
    fn test_add_nonce() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 2;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        let data = vec![0u8; 1024];
        let mut unstructured = Unstructured::new(&data[..]);

        for i in 0..num_operators {
            let random_nonce = Musig2PubNonce::arbitrary(&mut unstructured)
                .expect("should produce random pubnonce");

            let result = tx_state.add_nonce(&(i as u32), random_nonce);

            assert!(result.is_ok(), "Expected Ok since operator {} exists", i);

            if (i + 1) < num_operators {
                assert!(
                    !result.unwrap(),
                    "Expected false since not all nonces have been collected"
                );
            } else {
                assert!(
                    result.unwrap(),
                    "Expected true since all nonces have been collected"
                );
            }
        }

        let random_nonce =
            Musig2PubNonce::arbitrary(&mut unstructured).expect("should produce random pubnonce");
        let result = tx_state.add_nonce(&(num_operators as u32), random_nonce);

        assert!(
            result.is_err_and(|e| matches!(e, EntityError::BridgeOpUnauthorized)),
            "should result in `BridgeOpUnauthorized` error when adding nonce from an operator that is not part of the federation");
    }

    #[test]
    fn test_is_fully_signed_all_signatures_present() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 2;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        for i in 0..num_operators {
            let data = vec![0u8; 32];
            let mut unstructured = Unstructured::new(&data);
            let sig = Musig2PartialSig::arbitrary(&mut unstructured)
                .expect("should generate arbitrary signature");

            tx_state
                .add_signature(SignatureInfo::new(sig, i as u32), 0)
                .unwrap();
        }

        assert!(
            tx_state.is_fully_signed(),
            "Expected transaction to be fully signed"
        );
    }

    #[test]
    fn test_is_fully_signed_missing_signature() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 1;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        let data = vec![0u8; 32];
        let mut unstructured = Unstructured::new(&data);
        let sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate arbitrary signature");

        tx_state
            .add_signature(SignatureInfo::new(sig, 0), 0)
            .unwrap();

        assert!(
            tx_state.is_fully_signed(),
            "Expected transaction to be fully signed"
        );

        // Remove the signature and test again
        tx_state.collected_sigs[0].remove(&(own_index as u32));
        assert!(
            !tx_state.is_fully_signed(),
            "Expected transaction to not be fully signed"
        );
    }

    #[test]
    fn test_add_signature_success() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 1;
        let num_inputs = 3;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        let data = vec![0u8; 32];
        let mut unstructured = Unstructured::new(&data);
        let sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate arbitrary signature");

        for input_index in 0..num_inputs {
            assert!(tx_state
                .add_signature(
                    SignatureInfo::new(sig, own_index as OperatorIdx),
                    input_index
                )
                .is_ok());

            assert_eq!(
                tx_state.collected_sigs[input_index].get(&(own_index as u32)),
                Some(sig).as_ref()
            );
        }
    }

    #[test]
    fn test_add_signature_invalid_pubkey() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 1;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        let data = vec![0u8; 32];
        let mut unstructured = Unstructured::new(&data);
        let sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate arbitrary signature");

        let unauthorized_signer_index = num_operators + 1;
        let result =
            tx_state.add_signature(SignatureInfo::new(sig, unauthorized_signer_index as u32), 0);
        assert!(result.is_err());

        assert!(matches!(
            result.unwrap_err(),
            EntityError::BridgeTxStateError(BridgeTxStateError::Unauthorized),
        ));
    }

    #[test]
    fn test_add_signature_input_index_out_of_bounds() {
        let secp = Secp256k1::new();
        let own_index = 0;
        let num_operators = 1;
        let num_inputs = 1;
        let mut tx_state = create_mock_tx_state(&secp, own_index, num_inputs, num_operators);

        let data = vec![0u8; 32];
        let mut unstructured = Unstructured::new(&data);
        let sig = Musig2PartialSig::arbitrary(&mut unstructured)
            .expect("should generate arbitrary signature");

        let invalid_input_index = 1;
        let result = tx_state.add_signature(
            SignatureInfo::new(sig, own_index as u32),
            invalid_input_index,
        );
        assert!(result.is_err());

        let expected_txid = tx_state.unsigned_tx().compute_txid();
        let actual_error = result.unwrap_err();

        match actual_error {
            EntityError::BridgeTxStateError(BridgeTxStateError::TxinIdxOutOfBounds(
                actual_txid,
                actual_index,
            )) => {
                assert_eq!(actual_txid, expected_txid, "txid should match");
                assert_eq!(actual_index, invalid_input_index);
            }
            _ => {
                panic!(
                    "error should have BridgeSigEntityError::TxinIdxOutOfBounds but got: {}",
                    actual_error
                );
            }
        }
    }

    /// Creates a mock [`BridgeTxState`] for the given params. We do this manually here instead of
    /// leveraging [`arbitrary::Arbitrary`] since we want more fine-grained control over the created
    /// structure.
    fn create_mock_tx_state(
        secp: &Secp256k1<All>,
        own_index: usize,
        num_inputs: usize,
        num_operators: usize,
    ) -> BridgeTxState {
        let (pks, sks) = generate_keypairs(secp, num_operators);

        let tx_output = generate_mock_tx_signing_data(num_inputs);

        let pubkey_table = generate_pubkey_table(&pks);

        let sec_nonce =
            generate_sec_nonce(&tx_output.unsigned_tx.compute_txid(), pks, sks[own_index]);

        BridgeTxState::new(tx_output, pubkey_table, sec_nonce.into())
            .expect("Failed to create TxState")
    }
}
