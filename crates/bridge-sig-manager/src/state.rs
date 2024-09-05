//! Defines the [`TxState`] type that tracks the state of signature collection for a particular
//! [`Psbt`].

use std::collections::HashMap;

use alpen_express_primitives::bridge::{OperatorIdx, PublickeyTable};
use bitcoin::{secp256k1::schnorr::Signature, taproot::ControlBlock, Psbt, ScriptBuf, TxOut};
use express_bridge_tx_builder::prelude::TxSigningData;

use super::{errors::BridgeSigResult, signature::SignatureInfo};

/// The state a transaction is in with respect to the number of signatures that have been collected
/// from the bridge federation signatories.
#[derive(Debug, Clone)]
pub struct TxState {
    /// The partially signed bitcoin transaction that this state tracks.
    psbt: Psbt,

    /// The prevouts of the unsigned transaction required for taproot signing.
    prevouts: Vec<TxOut>,

    /// The witness elements required to spend a taproot output.
    spend_infos: Vec<(ScriptBuf, ControlBlock)>,

    /// The table of pubkeys that is used to lock the UTXO present as an input in the psbt.
    /// This table maps the `OperatorIdx` to their corresponding pubkeys.
    pubkey_table: PublickeyTable,

    /// The number of required signatures (same as the length of [`Self::pubkeys`])
    required_signatures: usize,

    /// The table of signatures collected so far per input and per operator.
    collected_sigs: Vec<HashMap<OperatorIdx, Signature>>,
}

impl TxState {
    /// Create a new [`TxState`] for the given [`Psbt`] and list of [`PublicKey`].
    pub fn new(
        tx_signing_data: TxSigningData,
        pubkey_table: PublickeyTable,
    ) -> BridgeSigResult<Self> {
        let num_keys = pubkey_table.0.len();
        let mut psbt = Psbt::from_unsigned_tx(tx_signing_data.unsigned_tx)?;
        let num_inputs = psbt.inputs.len();

        let prevouts = tx_signing_data.prevouts.clone();

        for (i, each_psbt) in psbt.inputs.iter_mut().enumerate() {
            each_psbt.witness_utxo = Some(prevouts[i].clone());
        }

        let collected_sigs = vec![HashMap::new(); num_inputs];

        Ok(Self {
            psbt,
            prevouts,
            spend_infos: tx_signing_data.spend_infos,
            required_signatures: num_keys,
            pubkey_table,
            collected_sigs,
        })
    }

    /// Get the [`Psbt`] that this state is associated with.
    pub fn psbt(&self) -> &Psbt {
        &self.psbt
    }

    /// Get the spend info associated with each input in the PSBT.
    pub fn spend_infos(&self) -> &[(ScriptBuf, ControlBlock)] {
        &self.spend_infos[..]
    }

    /// Get the relevant previous outputs of the transaction that this state tracks.
    pub fn prevouts(&self) -> &[TxOut] {
        &self.prevouts[..]
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

    /// Get table of signatures collected so far where the first index is the index of the
    /// transaction input and the second index is the index of the signer.
    pub fn collected_sigs(&self) -> &[HashMap<OperatorIdx, Signature>] {
        &self.collected_sigs[..]
    }

    /// Check if all the required signatures have been collected for the [`Psbt`].
    pub fn is_fully_signed(&self) -> bool {
        unimplemented!()
    }

    /// Add a signature to the collection. If the signature corresponding to a particular pubkey has
    /// already been added, it is re-added.
    ///
    /// # Errors
    ///
    /// If the [`SignatureInfo::signer_index`] is not a part of the required signatories or the
    /// `input_index` is not part of the [`Psbt`].
    pub fn add_signature(
        &mut self,
        _signature_info: SignatureInfo,
        _input_index: usize,
    ) -> BridgeSigResult<()> {
        unimplemented!()
    }
}
