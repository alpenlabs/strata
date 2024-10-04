mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use alpen_test_utils::bitcoin::get_btc_chain;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_sp1_adapter::SP1Verifier;
    use express_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{BtcBlockProofGenerator, ProofGenerator};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let prover_options = ProverOptions {
            enable_compression: true,
            use_mock_prover: false,
            ..Default::default()
        };

        let (proof, _) = BtcBlockProofGenerator::new()
            .get_proof(block, &prover_options)
            .unwrap();

        // TODO: add `extract_public_output_borsh` function to Verifier
        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");

        let _: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
