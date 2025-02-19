use strata_proofimpl_checkpoint::prover::{CheckpointProver, CheckpointProverInput};
use strata_test_utils::l2::gen_params;
use zkaleido::{AggregationInput, ZkVmHost, ZkVmResult};

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

#[derive(Debug)]
pub struct CheckpointBatchInfo {
    pub l1_range: (u64, u64),
    pub l2_range: (u64, u64),
}

impl<H: ZkVmHost> ProofGenerator for CheckpointProofGenerator<H> {
    type Input = CheckpointBatchInfo;
    type P = CheckpointProver;
    type H = H;

    fn get_input(&self, batch_info: &CheckpointBatchInfo) -> ZkVmResult<CheckpointProverInput> {
        let params = gen_params();
        let rollup_params = params.rollup();

        let (l1_start_height, l1_end_height) = batch_info.l1_range;
        let (l2_start_height, l2_end_height) = batch_info.l2_range;
        let cl_batches = vec![(l2_start_height, l2_end_height)];

        let l1_batch_proof = self
            .l1_batch_prover
            .get_proof(&(l1_start_height, l1_end_height))
            .unwrap();
        let l1_batch_vk = self.l1_batch_prover.get_host().get_verification_key();
        let l1_batch = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let l2_batch_proof = self.l2_batch_prover.get_proof(&cl_batches).unwrap();
        let l2_batch_vk = self.l2_batch_prover.get_host().get_verification_key();
        let l2_batch = AggregationInput::new(l2_batch_proof, l2_batch_vk);

        let input = CheckpointProverInput {
            rollup_params: rollup_params.clone(),
            l1_batch,
            l2_batch,
        };

        Ok(input)
    }

    fn get_proof_id(&self, info: &CheckpointBatchInfo) -> String {
        let (l1_start_height, l1_end_height) = info.l1_range;
        let (l2_start_height, l2_end_height) = info.l2_range;
        format!(
            "checkpoint_l1_{}_{}_l2_{}_{}",
            l1_start_height, l1_end_height, l2_start_height, l2_end_height
        )
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[allow(dead_code)]
fn test_proof<H: ZkVmHost>(checkpoint_prover: &CheckpointProofGenerator<H>) {
    let params = gen_params();
    let rollup_params = params.rollup();
    let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
    let l1_end_height = l1_start_height + 2;

    let l2_start_height = 1;
    let l2_end_height = 3;

    let prover_input = CheckpointBatchInfo {
        l1_range: (l1_start_height.into(), l1_end_height.into()),
        l2_range: (l2_start_height, l2_end_height),
    };

    let _ = checkpoint_prover
        .get_proof(&prover_input)
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
