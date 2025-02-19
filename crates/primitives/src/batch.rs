use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use zkaleido::{Proof, ProofReceipt, PublicValues};

use crate::{
    block_credential::CredRule,
    buf::{Buf32, Buf64},
    crypto::verify_schnorr_sig,
    epoch::EpochCommitment,
    hash,
    l1::L1BlockCommitment,
    l2::{L2BlockCommitment, L2BlockId},
    params::RollupParams,
};

/// Summary generated when we accept the last block of an epoch.
///
/// It's possible in theory for more than one of these to validly exist for a
/// single epoch, but not in the same chain.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Arbitrary,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
pub struct EpochSummary {
    /// The epoch number.
    ///
    /// These are always sequential.
    epoch: u64,

    /// The last block of the checkpoint.
    terminal: L2BlockCommitment,

    /// The previous epoch that this epoch was built on.
    ///
    /// If this is the genesis epoch, then this is all zero.
    prev_terminal: L2BlockCommitment,

    /// The new L1 block that was submitted in the terminal block.
    new_l1: L1BlockCommitment,

    /// The final state root of the epoch.
    ///
    /// Currently this is just copied from the state root of the header of the
    /// last block of the slot, but it's likely we'll change this to add
    /// processing outside of the terminal block before "finishing" the epoch.
    final_state: Buf32,
}

impl EpochSummary {
    /// Creates a new instance.
    pub fn new(
        epoch: u64,
        terminal: L2BlockCommitment,
        prev_terminal: L2BlockCommitment,
        new_l1: L1BlockCommitment,
        final_state: Buf32,
    ) -> Self {
        Self {
            epoch,
            terminal,
            prev_terminal,
            new_l1,
            final_state,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn terminal(&self) -> &L2BlockCommitment {
        &self.terminal
    }

    pub fn prev_terminal(&self) -> &L2BlockCommitment {
        &self.prev_terminal
    }

    pub fn new_l1(&self) -> &L1BlockCommitment {
        &self.new_l1
    }

    pub fn final_state(&self) -> &Buf32 {
        &self.final_state
    }

    /// Generates an epoch commitent for this epoch using the data in the
    /// summary.
    pub fn get_epoch_commitment(&self) -> EpochCommitment {
        EpochCommitment::new(self.epoch, self.terminal.slot(), *self.terminal.blkid())
    }

    /// Gets the epoch commitment for the previous epoch, using the terminal
    /// block reference the header stores.
    pub fn get_prev_epoch_commitment(self) -> Option<EpochCommitment> {
        if self.epoch == 0 {
            None
        } else {
            Some(EpochCommitment::new(
                self.epoch - 1,
                self.prev_terminal.slot(),
                *self.prev_terminal.blkid(),
            ))
        }
    }

    /// Create the summary for the next epoch based on this one.
    pub fn create_next_epoch_summary(
        &self,
        new_terminal: L2BlockCommitment,
        new_l1: L1BlockCommitment,
        new_state: Buf32,
    ) -> EpochSummary {
        Self::new(
            self.epoch() + 1,
            new_terminal,
            *self.terminal(),
            new_l1,
            new_state,
        )
    }
}

/// Consolidates all information required to describe and verify a batch checkpoint.
/// This includes metadata about the batch, the state transitions, checkpoint base state,
/// and the proof itself. The proof verifies that the transition in [`BatchTransition`]
/// is valid for the batch described by [`BatchInfo`].
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct Checkpoint {
    /// Information regarding the current batches of l1 and l2 blocks along with epoch.
    batch_info: BatchInfo,

    /// Transition data for L1 and L2 states, which is verified by the proof.
    transition: BatchTransition,

    /// Reference state commitment against which batch transition and corresponding proof is
    /// verified
    base_state_commitment: BaseStateCommitment,

    /// Proof for the batch obtained from prover manager
    proof: Proof,
}

impl Checkpoint {
    pub fn new(
        batch_info: BatchInfo,
        transition: BatchTransition,
        base_state_commitment: BaseStateCommitment,
        proof: Proof,
    ) -> Self {
        Self {
            batch_info,
            transition,
            base_state_commitment,
            proof,
        }
    }

    pub fn batch_info(&self) -> &BatchInfo {
        &self.batch_info
    }

    pub fn batch_transition(&self) -> &BatchTransition {
        &self.transition
    }

    pub fn base_state_commitment(&self) -> &BaseStateCommitment {
        &self.base_state_commitment
    }

    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    pub fn set_proof(&mut self, proof: Proof) {
        self.proof = proof
    }

    /// Returns the output that should be expected from the checkpoint proof
    /// verification.
    pub fn get_proof_output(&self) -> CheckpointProofOutput {
        CheckpointProofOutput::new(
            self.batch_transition().clone(),
            self.base_state_commitment().clone(),
        )
    }

    #[deprecated(note = "use `checkpoint_verification::construct_receipt`")]
    pub fn get_proof_receipt(&self) -> ProofReceipt {
        let proof = self.proof().clone();
        let output = self.get_proof_output();
        let public_values =
            PublicValues::new(borsh::to_vec(&output).expect("checkpoint: proof output"));
        ProofReceipt::new(proof, public_values)
    }

    pub fn hash(&self) -> Buf32 {
        // FIXME make this more structured and use incremental hashing

        let mut buf = vec![];
        let batch_serialized =
            borsh::to_vec(&self.batch_info).expect("could not serialize checkpoint info");

        buf.extend(&batch_serialized);
        buf.extend(self.proof.as_bytes());

        hash::raw(&buf)
    }
}

#[derive(
    Clone, Debug, BorshDeserialize, BorshSerialize, Arbitrary, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct SignedCheckpoint {
    inner: Checkpoint,
    signature: Buf64,
}

impl SignedCheckpoint {
    pub fn new(inner: Checkpoint, signature: Buf64) -> Self {
        Self { inner, signature }
    }

    pub fn checkpoint(&self) -> &Checkpoint {
        &self.inner
    }

    pub fn signature(&self) -> &Buf64 {
        &self.signature
    }
}

impl From<SignedCheckpoint> for Checkpoint {
    fn from(value: SignedCheckpoint) -> Self {
        value.inner
    }
}

/// Contains metadata describing a batch checkpoint, including the L1 and L2 height ranges
/// it covers and the final L2 block ID in that range.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchInfo {
    /// Checkpoint epoch
    pub epoch: u64,

    /// L1 block range(inclusive) the checkpoint covers
    pub l1_range: (L1BlockCommitment, L1BlockCommitment),

    /// L2 block range(inclusive) the checkpoint covers
    pub l2_range: (L2BlockCommitment, L2BlockCommitment),
}

impl BatchInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: (L1BlockCommitment, L1BlockCommitment),
        l2_range: (L2BlockCommitment, L2BlockCommitment),
    ) -> Self {
        Self {
            epoch: checkpoint_idx,
            l1_range,
            l2_range,
        }
    }

    /// Geets the epoch index.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Gets the epoch commitment for this batch.
    pub fn get_epoch_commitment(&self) -> EpochCommitment {
        EpochCommitment::from_terminal(self.epoch(), *self.final_l2_block())
    }

    /// Gets the final L2 block commitment in the batch's L2 range.
    pub fn final_l2_block(&self) -> &L2BlockCommitment {
        &self.l2_range.1
    }

    /// Gets the final L2 blkid in the batch's L2 range.
    pub fn final_l2_blockid(&self) -> &L2BlockId {
        self.l2_range.1.blkid()
    }

    /// Gets the final L1 block commitment in the batch's L1 range.
    pub fn final_l1_block(&self) -> &L1BlockCommitment {
        &self.l1_range.1
    }

    /// Check is whether the L2 slot is covered by the checkpoint
    pub fn includes_l2_block(&self, slot: u64) -> bool {
        let (_, last_l2_commitment) = self.l2_range;
        if slot <= last_l2_commitment.slot() {
            return true;
        }
        false
    }

    /// check for whether the L1 height is covered by the checkpoint
    pub fn includes_l1_block(&self, height: u64) -> bool {
        let (_, last_l1_commitment) = self.l1_range;
        if height <= last_l1_commitment.height() {
            return true;
        }
        false
    }
}

/// Describes state transitions for both L1 and L2, along with a commitment to the
/// rollup parameters. The proof associated with the batch verifies this transition.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchTransition {
    /// The inclusive hash range of `HeaderVerificationState` for L1 blocks.
    ///
    /// Represents a transition from the starting L1 state to the ending L1 state.
    /// The hash is computed via [`super::l1::HeaderVerificationState::compute_hash`].
    pub l1_transition: (Buf32, Buf32),

    /// The inclusive hash range of `Chainstate` for L2 blocks.
    ///
    /// Represents a transition from the starting L2 state to the ending L2 state.
    /// The state root is computed via [`super::chain_state::Chainstate::compute_state_root`].
    pub l2_transition: (Buf32, Buf32),

    /// A commitment to the `RollupParams`, as computed by `RollupParams::compute_hash`.
    ///
    /// This indicates that the transition is valid under these rollup parameters.
    pub rollup_params_commitment: Buf32,
}

impl BatchTransition {
    pub fn new(
        l1_transition: (Buf32, Buf32),
        l2_transition: (Buf32, Buf32),
        rollup_params_commitment: Buf32,
    ) -> Self {
        Self {
            l1_transition,
            l2_transition,
            rollup_params_commitment,
        }
    }

    /// Creates a [`BaseStateCommitment`] by taking the initial state of the
    /// [`BatchTransition`]
    pub fn get_initial_base_state_commitment(&self) -> BaseStateCommitment {
        BaseStateCommitment::new(self.l1_transition.0, self.l2_transition.0)
    }

    /// Creates a [`BaseStateCommitment`] by taking the final state of the
    /// [`BatchTransition`]
    pub fn get_final_base_state_commitment(&self) -> BaseStateCommitment {
        BaseStateCommitment::new(self.l1_transition.1, self.l2_transition.1)
    }

    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

/// Represents the reference state commitment against which batch transitions and proofs are
/// verified.
///
/// NOTE/TODO: This state serves as the starting point for verifying a checkpoint proof. If we move
/// towards a strict mode where we prove each checkpoint recursively, this should be replaced with
/// `GenesisState`.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BaseStateCommitment {
    pub initial_l1_state: Buf32,
    pub initial_l2_state: Buf32,
}

impl BaseStateCommitment {
    pub fn new(initial_l1_state: Buf32, initial_l2_state: Buf32) -> Self {
        Self {
            initial_l1_state,
            initial_l2_state,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct CheckpointProofOutput {
    pub batch_transition: BatchTransition,
    pub base_state_commitment: BaseStateCommitment,
}

impl CheckpointProofOutput {
    pub fn new(
        batch_transition: BatchTransition,
        base_state_commitment: BaseStateCommitment,
    ) -> CheckpointProofOutput {
        Self {
            batch_transition,
            base_state_commitment,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct CommitmentInfo {
    pub blockhash: Buf32,
    pub txid: Buf32,
    pub wtxid: Buf32,
    pub block_height: u64,
    pub position: u32,
}

impl CommitmentInfo {
    pub fn new(
        blockhash: Buf32,
        txid: Buf32,
        wtxid: Buf32,
        block_height: u64,
        position: u32,
    ) -> Self {
        Self {
            blockhash,
            txid,
            wtxid,
            block_height,
            position,
        }
    }
}

/// Contains the checkpoint data along with its commitment to l1.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct L1CommittedCheckpoint {
    /// The actual `Checkpoint` data.
    pub checkpoint: Checkpoint,
    /// Its commitment to L1 used to locate/identify the checkpoint in L1.
    pub commitment: CommitmentInfo,
}

impl L1CommittedCheckpoint {
    pub fn new(checkpoint: Checkpoint, commitment: CommitmentInfo) -> Self {
        Self {
            checkpoint,
            commitment,
        }
    }
}

/// Verifies that a signed checkpoint has a proper signature according to rollup
/// params.
// TODO this might want to take a chainstate in the future, but we don't have
// the ability to get that where we call this yet
pub fn verify_signed_checkpoint_sig(
    signed_checkpoint: &SignedCheckpoint,
    params: &RollupParams,
) -> bool {
    let seq_pubkey = match params.cred_rule {
        CredRule::SchnorrKey(key) => key,

        // In this case we always just assume true.
        CredRule::Unchecked => return true,
    };

    let checkpoint_sighash = signed_checkpoint.checkpoint().hash();

    verify_schnorr_sig(
        &signed_checkpoint.signature(),
        &checkpoint_sighash,
        &seq_pubkey,
    )
}
