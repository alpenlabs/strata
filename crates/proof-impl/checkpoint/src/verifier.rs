use strata_zkvm::ZkVmVerifier;

use crate::CheckpointProofOutput;

pub struct CheckpointVerifier;

impl ZkVmVerifier for CheckpointVerifier {
    type Output = CheckpointProofOutput;

    fn get_raw_public_output(output: &Self::Output) -> anyhow::Result<Vec<u8>> {
        Ok(borsh::to_vec(output)?)
    }
}
