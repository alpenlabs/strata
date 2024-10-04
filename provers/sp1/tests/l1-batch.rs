mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_l1_batch::L1BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_test_utils::l2::gen_params;
    use strata_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{BtcBlockProofGenerator, L1BatchProofGenerator, ProofGenerator};

    #[test]
    fn test_l1_batch_code_trace_generation() {
        sp1_sdk::utils::setup_logger();

        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 2;

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: true,
        };

        let btc_proof_generator = BtcBlockProofGenerator::new();
        let (proof, _) = L1BatchProofGenerator::new(btc_proof_generator)
            .get_proof(&(l1_start_height, l1_end_height), &prover_options)
            .unwrap();

        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: L1BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
