mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use express_proofimpl_checkpoint::L2BatchProofOutput;
    use express_sp1_adapter::SP1Verifier;
    use express_zkvm::{ProverOptions, ZKVMVerifier};

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
        };

        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover);

        let (proof, _) = cl_agg_prover.get_proof(&(1, 3), &prover_options).unwrap();

        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: L2BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
