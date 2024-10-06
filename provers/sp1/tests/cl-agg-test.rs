mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_checkpoint::L2BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{
        ClProofGenerator, ElProofGenerator, L2BatchProofGenerator, ProofGenerator,
    };

    #[test]
    fn test_cl_agg_guest_code_trace_generation() {
        sp1_sdk::utils::setup_logger();

        let prover_options = ProverOptions {
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_mock_prover: false,
            use_cached_keys: true,
        };

        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover);

        let (proof, _) = cl_agg_prover.get_proof(&(1, 3), &prover_options).unwrap();

        let _: L2BatchProofOutput = SP1Verifier::extract_borsh_public_output(&proof)
            .expect("Failed to extract public outputs");
    }
}
