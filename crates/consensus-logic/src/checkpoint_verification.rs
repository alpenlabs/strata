//! General handling around checkpoint verification.

use strata_primitives::{params::*, proof::RollupVerifyingKey};
use strata_state::{batch::*, client_state::L1Checkpoint};
use thiserror::Error;
use tracing::*;
use zkaleido::{ProofReceipt, ZkVmError, ZkVmResult};
use zkaleido_risc0_adapter;
use zkaleido_sp1_adapter;

use crate::errors::CheckpointError;

// FIXME this isn't really an extension trait since it's not being used to
// blanket impl over another trait
/// Extends [`RollupVerifyingKey`] with verification logic.
pub trait VerifyingKeyExt {
    fn verify_groth16(&self, proof_receipt: &ProofReceipt) -> ZkVmResult<()>;
}

impl VerifyingKeyExt for RollupVerifyingKey {
    fn verify_groth16(&self, proof_receipt: &ProofReceipt) -> ZkVmResult<()> {
        // NOTE/TODO: this should also verify that this checkpoint is based on top of some previous
        // checkpoint
        match self {
            RollupVerifyingKey::Risc0VerifyingKey(vk) => {
                zkaleido_risc0_adapter::verify_groth16(proof_receipt, vk.as_ref())
            }
            RollupVerifyingKey::SP1VerifyingKey(vk) => {
                zkaleido_sp1_adapter::verify_groth16(proof_receipt, vk.as_ref())
            }
            // In Native Execution mode, we do not actually generate the proof to verify. Checking
            // public parameters is sufficient.
            RollupVerifyingKey::NativeVerifyingKey(_) => Ok(()),
        }
    }
}

/// Verifies if a checkpoint if valid, given the context of a previous checkpoint.
///
/// If this is the first checkpoint we verify, then there is no checkpoint to
/// check against.
// TODO reduce this to actually just passing in the core information we really
// need, not like the height
pub fn verify_checkpoint(
    checkpoint: &Checkpoint,
    prev_checkpoint: Option<&L1Checkpoint>,
    params: &RollupParams,
) -> Result<(), CheckpointError> {
    // First thing obviously is to verify the proof.  No sense in continuing if
    // the proof is invalid.
    let proof_receipt = construct_receipt(checkpoint);
    verify_proof(checkpoint, &proof_receipt, params)?;

    // Then also verify the signature.
    // TODO verify signature.

    // And check that we're building upon the previous state correctly.
    if let Some(prev) = prev_checkpoint {
        verify_checkpoint_extends(checkpoint, prev, params)?;
    } else {
        // If it's the first checkpoint we want it to be the initial epoch.
        if checkpoint.batch_info().epoch() != 0 {
            return Err(CheckpointError::SkippedGenesis);
        }
    }

    Ok(())
}

/// Verifies that the a checkpoint extends the state of a previous checkpoint.
fn verify_checkpoint_extends(
    checkpoint: &Checkpoint,
    prev: &L1Checkpoint,
    params: &RollupParams,
) -> Result<(), CheckpointError> {
    let epoch = checkpoint.batch_info().epoch();
    let prev_epoch = prev.batch_info.epoch();
    let last_l1_tsn = prev.batch_transition.l1_transition;
    let last_l2_tsn = prev.batch_transition.l2_transition;
    let l1_tsn = checkpoint.batch_transition().l1_transition;
    let l2_tsn = checkpoint.batch_transition().l2_transition;

    // Check that the epoch numbers line up.
    if epoch != prev_epoch + 1 {
        return Err(CheckpointError::Sequencing(epoch, prev_epoch));
    }

    // Check that the L1 blocks match up.
    if l1_tsn.0 != last_l1_tsn.1 {
        warn!("checkpoint mismatch on L1 state!");
        return Err(CheckpointError::MismatchL1State);
    }

    if l2_tsn.0 != last_l2_tsn.1 {
        warn!("checkpoint mismatch on L2 state!");
        return Err(CheckpointError::MismatchL2State);
    }

    Ok(())
}

/// Constructs a receipt from a checkpoint.
///
/// This is here because we want to move `.get_proof_receipt()` out of the
/// checkpoint type itself soon.
fn construct_receipt(checkpoint: &Checkpoint) -> ProofReceipt {
    #[allow(deprecated)]
    checkpoint.get_proof_receipt()
}

/// Verify that the provided checkpoint proof is valid for the verifier key.
///
/// # Caution
///
/// If the checkpoint proof is empty, this function returns an `Ok(())`.
pub fn verify_proof(
    checkpoint: &Checkpoint,
    proof_receipt: &ProofReceipt,
    rollup_params: &RollupParams,
) -> ZkVmResult<()> {
    let rollup_vk = rollup_params.rollup_vk;
    let checkpoint_idx = checkpoint.batch_info().epoch();
    info!(%checkpoint_idx, "verifying proof");

    // FIXME: we are accepting empty proofs for now (devnet) to reduce dependency on the prover
    // infra.
    if rollup_params.proof_publish_mode.allow_empty()
        && proof_receipt.proof().is_empty()
        && proof_receipt.public_values().is_empty()
    {
        warn!(%checkpoint_idx, "verifying empty proof as correct");
        return Ok(());
    }

    let expected_public_output = checkpoint.get_proof_output();
    let actual_public_output: CheckpointProofOutput =
        borsh::from_slice(proof_receipt.public_values().as_bytes())
            .map_err(|e| ZkVmError::OutputExtractionError { source: e.into() })?;

    if expected_public_output != actual_public_output {
        dbg!(actual_public_output, expected_public_output);
        return Err(ZkVmError::ProofVerificationError(
            "Public output mismatch during proof verification".to_string(),
        ));
    }

    rollup_vk.verify_groth16(proof_receipt)
}
