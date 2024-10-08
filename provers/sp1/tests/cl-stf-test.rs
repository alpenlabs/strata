mod helpers;

#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_checkpoint::L2BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{ClProofGenerator, ElProofGenerator, ProofGenerator};

    #[test]
    fn test_cl_stf_guest_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let height = 1;

        let prover_ops = ProverOptions {
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_mock_prover: false,
            use_cached_keys: true,
        };
        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);

        let (proof, _) = cl_prover.get_proof(&height, &prover_ops).unwrap();

        let _: L2BatchProofOutput = SP1Verifier::extract_borsh_public_output(&proof)
            .expect("Failed to extract public outputs");
    }
}
