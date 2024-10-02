// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
mod helpers;
// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use express_proofimpl_l1_batch::L1BatchProofOutput;
    use express_sp1_adapter::SP1Verifier;
    use express_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::get_l1_batch_proof;

    #[test]
    fn test_l1_batch_code_trace_generation() {
        sp1_sdk::utils::setup_logger();

        let _prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: true,
        };

        let (proof, _) = get_l1_batch_proof(40321, 40324).unwrap();

        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: L1BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
