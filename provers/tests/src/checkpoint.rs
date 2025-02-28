use strata_proofimpl_checkpoint::program::{CheckpointProgram, CheckpointProverInput};
use zkaleido::{ZkVmHost, ZkVmResult};

use super::ProofGenerator;
use crate::cl::ClProofGenerator;

#[derive(Clone)]
pub struct CheckpointProofGenerator<H: ZkVmHost> {
    cl_stf_prover: ClProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> CheckpointProofGenerator<H> {
    pub fn new(cl_stf_prover: ClProofGenerator<H>, host: H) -> Self {
        Self {
            cl_stf_prover,
            host,
        }
    }
}

impl<H: ZkVmHost> ProofGenerator for CheckpointProofGenerator<H> {
    type Input = (u64, u64); // L2 Range
    type P = CheckpointProgram;
    type H = H;

    fn get_input(&self, l2_range: &(u64, u64)) -> ZkVmResult<CheckpointProverInput> {
        let cl_stf_proofs = vec![self.cl_stf_prover.get_proof(l2_range).unwrap()];
        let cl_stf_vk = self.cl_stf_prover.get_host().vk();

        let input = CheckpointProverInput {
            cl_stf_proofs,
            cl_stf_vk,
        };

        Ok(input)
    }

    fn get_proof_id(&self, l2_range: &(u64, u64)) -> String {
        let (l2_start_height, l2_end_height) = l2_range;
        format!("checkpoint_l2_{}_{}", l2_start_height, l2_end_height)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[allow(dead_code)]
fn test_proof<H: ZkVmHost>(checkpoint_prover: &CheckpointProofGenerator<H>) {
    let l2_start_height = 1;
    let l2_end_height = 3;

    let _ = checkpoint_prover
        .get_proof(&(l2_start_height, l2_end_height))
        .expect("Failed to generate proof");
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.checkpoint());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.checkpoint());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.checkpoint());
    }
}
