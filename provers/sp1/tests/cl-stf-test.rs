mod helpers;

// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_checkpoint::L2BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_zkvm::{ProverOptions, ZkVmVerifier};

    use crate::helpers::{ClProofGenerator, ElProofGenerator, ProofGenerator};

    #[test]
    fn test_cl_stf_guest_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let height = 1;

        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);

        let _ = cl_prover.get_proof(&height).unwrap();
    }
}
