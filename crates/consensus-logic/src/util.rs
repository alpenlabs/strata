use strata_primitives::{params::RollupParams, proof::RollupVerifyingKey};
use strata_risc0_adapter;
use strata_sp1_adapter;
use strata_state::batch::{BatchCheckpoint, CheckpointProofOutput};
use strata_zkvm::{ProofReceipt, ZkVmError, ZkVmResult};
use tracing::*;

/// Verify that the provided checkpoint proof is valid for the verifier key.
///
/// # Caution
///
/// If the checkpoint proof is empty, this function returns an `Ok(())`.
pub fn verify_proof(
    checkpoint: &BatchCheckpoint,
    proof_receipt: &ProofReceipt,
    rollup_params: &RollupParams,
) -> ZkVmResult<()> {
    let rollup_vk = rollup_params.rollup_vk;
    let checkpoint_idx = checkpoint.batch_info().idx();
    let proof = checkpoint.proof();
    info!(%checkpoint_idx, "verifying proof");

    // FIXME: we are accepting empty proofs for now (devnet) to reduce dependency on the prover
    // infra.
    if rollup_params.proof_publish_mode.allow_empty() && proof_receipt.is_empty() {
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
    let public_params_raw = proof_receipt.public_values().as_bytes();

    // NOTE/TODO: this should also verify that this checkpoint is based on top of some previous
    // checkpoint
    match rollup_vk {
        RollupVerifyingKey::Risc0VerifyingKey(vk) => {
            strata_risc0_adapter::verify_groth16(proof, vk.as_ref(), public_params_raw)
        }
        RollupVerifyingKey::SP1VerifyingKey(vk) => {
            strata_sp1_adapter::verify_groth16(proof, vk.as_ref(), public_params_raw)
        }
        // In Native Execution mode, we do not actually generate the proof to verify. Checking
        // public parameters is sufficient.
        RollupVerifyingKey::NativeVerifyingKey(_) => Ok(()),
    }
}
