use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use zkaleido::{ZkVmHost, ZkVmResult};

use super::{cl::ClProofGenerator, ProofGenerator};

#[derive(Clone)]
pub struct L2BatchProofGenerator<H: ZkVmHost> {
    cl_proof_generator: ClProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> L2BatchProofGenerator<H> {
    pub fn new(cl_proof_generator: ClProofGenerator<H>, host: H) -> Self {
        Self {
            cl_proof_generator,
            host,
        }
    }
}

impl<H: ZkVmHost> ProofGenerator for L2BatchProofGenerator<H> {
    type Input = Vec<(u64, u64)>;
    type P = ClAggProver;
    type H = H;

    fn get_input(&self, batches: &Self::Input) -> ZkVmResult<ClAggInput> {
        let mut batch = Vec::new();

        for mini_batch_range in batches {
            let (start_height, end_height) = *mini_batch_range;
            let cl_proof = self
                .cl_proof_generator
                .get_proof(&(start_height, end_height))?;
            batch.push(cl_proof);
        }

        let cl_stf_vk = self.cl_proof_generator.get_host().get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }

    fn get_proof_id(&self, batches: &Self::Input) -> String {
        if let (Some(first), Some(last)) = (batches.first(), batches.last()) {
            format!("cl_batch_{}_{}", first.0, last.1)
        } else {
            "cl_batch_empty".to_string()
        }
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proof<H: ZkVmHost>(cl_agg_prover: &L2BatchProofGenerator<H>) {
        let _ = cl_agg_prover.get_proof(&vec![(1, 3)]).unwrap();
    }

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.l2_batch());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.l2_batch());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.l2_batch());
    }
}
