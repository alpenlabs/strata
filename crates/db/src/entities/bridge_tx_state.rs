//! Defines the [`TxState`] type that tracks the state of signature collection for a particular
//! [`Psbt`].

use std::collections::HashMap;

use alpen_express_primitives::{
    bridge::{OperatorIdx, PublickeyTable, SchnorrSignature, SignatureInfo, TxSigningData},
    l1::{BitcoinPsbt, BitcoinTxOut, SpendInfo},
};
use arbitrary::Arbitrary;
use bitcoin::{Psbt, Transaction, TxOut, Txid};
use borsh::{BorshDeserialize, BorshSerialize};

use super::errors::{BridgeTxStateError, EntityResult};

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

    /// The table of signatures collected so far per input and per operator.
    collected_sigs: Vec<HashMap<OperatorIdx, SchnorrSignature>>,
}

impl BridgeTxState {
    /// Create a new [`TxState`] for the given [`Psbt`] and list of [`PublicKey`].
    pub fn new(tx_signing_data: TxSigningData, pubkey_table: PublickeyTable) -> EntityResult<Self> {
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

        Ok(Self {
            psbt: psbt.into(),
            prevouts,
            spend_infos: tx_signing_data.spend_infos,
            required_signatures: num_keys,
            pubkey_table,
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

    /// Get the list of [`PublicKey`]'s in the locking script of the UTXO that the [`Psbt`]
    /// spends.
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

    /// Get table of signatures collected so far where the first index is the index of the
    /// transaction input and the second index is the index of the signer.
    pub fn collected_sigs(&self) -> &[HashMap<OperatorIdx, SchnorrSignature>] {
        &self.collected_sigs[..]
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
        self.pubkey_table
            .0
            .get(signer_index)
            .ok_or(BridgeTxStateError::Unauthorized)?;

        self.collected_sigs[input_index].insert(*signer_index, *signature_info.signature());

        Ok(self.is_fully_signed())
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::bridge::{
        generate_keypairs, generate_mock_prevouts, generate_pubkey_table,
    };
    use bitcoin::{
        absolute::{self},
        key::{Keypair, Secp256k1},
        secp256k1::Message,
        transaction::Version,
        Transaction,
    };

    use super::*;
    use crate::entities::errors::EntityError;

    fn create_test_tx_output(inputs: usize) -> TxSigningData {
        let tx = Transaction {
            version: Version(1),
            lock_time: absolute::LockTime::ZERO,
            input: vec![Default::default(); inputs],
            output: vec![],
        };

        let prevouts = generate_mock_prevouts(inputs);

        TxSigningData {
            unsigned_tx: tx,
            prevouts,
            spend_infos: vec![],
        }
    }

    #[test]
    fn test_is_fully_signed_all_signatures_present() {
        let secp = Secp256k1::new();
        let (pks, sks) = generate_keypairs(&secp, 2);

        let tx_output = create_test_tx_output(1); // Single input
        let pubkey_table = generate_pubkey_table(&pks);
        let mut tx_state =
            BridgeTxState::new(tx_output, pubkey_table).expect("Failed to create TxState");

        let sig1 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[0]),
        );
        let sig2 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[1]),
        );

        tx_state
            .add_signature(SignatureInfo::new(sig1.into(), 0), 0)
            .unwrap();
        tx_state
            .add_signature(SignatureInfo::new(sig2.into(), 1), 0)
            .unwrap();

        assert!(
            tx_state.is_fully_signed(),
            "Expected transaction to be fully signed"
        );
    }

    #[test]
    fn test_is_fully_signed_missing_signature() {
        let secp = Secp256k1::new();
        let (pks, sks) = generate_keypairs(&secp, 1);

        let tx_output = create_test_tx_output(1); // Single input

        let pubkey_table = generate_pubkey_table(&pks);
        let mut tx_state =
            BridgeTxState::new(tx_output, pubkey_table).expect("Failed to create TxState");

        let sig1 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[0]),
        );

        tx_state
            .add_signature(SignatureInfo::new(sig1.into(), 0), 0)
            .unwrap();

        assert!(
            tx_state.is_fully_signed(),
            "Expected transaction to be fully signed"
        );

        // Remove the signature and test again
        tx_state.collected_sigs[0].remove(&0);
        assert!(
            !tx_state.is_fully_signed(),
            "Expected transaction to not be fully signed"
        );
    }

    #[test]
    fn test_add_signature_success() {
        let secp = Secp256k1::new();
        let (pks, sks) = generate_keypairs(&secp, 1);

        let tx_output = create_test_tx_output(1); // Single input
        let pubkey_table = generate_pubkey_table(&pks);
        let mut tx_state =
            BridgeTxState::new(tx_output, pubkey_table).expect("Failed to create TxState");

        let sig1 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[0]),
        );

        assert!(tx_state
            .add_signature(SignatureInfo::new(sig1.into(), 0), 0)
            .is_ok());

        assert_eq!(
            tx_state.collected_sigs[0].get(&0),
            Some(sig1.into()).as_ref()
        );
    }

    #[test]
    fn test_add_signature_invalid_pubkey() {
        let secp = Secp256k1::new();
        let (pks, sks) = generate_keypairs(&secp, 1);

        let tx_output = create_test_tx_output(1); // Single input
        let pubkey_table = generate_pubkey_table(&pks);
        let mut tx_state =
            BridgeTxState::new(tx_output, pubkey_table).expect("Failed to create TxState");

        let sig1 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[0]),
        );

        let unauthorized_signer_index = (pks.len() + 1) as u32;
        let result = tx_state.add_signature(
            SignatureInfo::new(sig1.into(), unauthorized_signer_index),
            0,
        );
        assert!(result.is_err());

        assert!(matches!(
            result.unwrap_err(),
            EntityError::BridgeTxStateError(BridgeTxStateError::Unauthorized),
        ));
    }

    #[test]
    fn test_add_signature_input_index_out_of_bounds() {
        let secp = Secp256k1::new();
        let (pks, sks) = generate_keypairs(&secp, 1);

        let tx_output = create_test_tx_output(1); // Single input
        let pubkey_table = generate_pubkey_table(&pks);
        let mut tx_state =
            BridgeTxState::new(tx_output, pubkey_table).expect("Failed to create TxState");

        let sig1 = secp.sign_schnorr(
            &Message::from_digest_slice(&[0xab; 32]).unwrap(),
            &Keypair::from_secret_key(&secp, &sks[0]),
        );

        let invalid_input_index = 1;
        let result =
            tx_state.add_signature(SignatureInfo::new(sig1.into(), 0), invalid_input_index);
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
}
