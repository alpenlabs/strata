use strata_primitives::proof::RollupVerifyingKey;
use zkaleido::{ProofReceipt, ZkVmResult};

pub fn verify_rollup_groth16_proof_receipt(
    proof_receipt: &ProofReceipt,
    rollup_vk: &RollupVerifyingKey,
) -> ZkVmResult<()> {
    match rollup_vk {
        RollupVerifyingKey::Risc0VerifyingKey(vk) => {
            zkaleido_risc0_groth16_verifier::verify_groth16(proof_receipt, vk.as_ref())
        }
        RollupVerifyingKey::SP1VerifyingKey(vk) => {
            zkaleido_sp1_groth16_verifier::verify_groth16(proof_receipt, vk.as_ref())
        }
        // In Native Execution mode, we do not actually generate the proof to verify. Checking
        // public parameters is sufficient.
        RollupVerifyingKey::NativeVerifyingKey(_) => Ok(()),
    }
}
