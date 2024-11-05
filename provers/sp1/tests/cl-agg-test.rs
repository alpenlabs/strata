mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_checkpoint::L2BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_zkvm::{ProverOptions, ZkVmVerifier};

    use crate::helpers::{
        ClProofGenerator, ElProofGenerator, L2BatchProofGenerator, ProofGenerator,
    };

    #[test]
    fn test_cl_agg_guest_code_trace_generation() {
        sp1_sdk::utils::setup_logger();

        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover);

        let _ = cl_agg_prover.get_proof(&(1, 3)).unwrap();
    }
}
