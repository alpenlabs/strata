mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use alpen_test_utils::{bitcoin::get_btc_chain, l2::gen_params};
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_sp1_adapter::SP1Verifier;
    use express_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{BtcBlockProofGenerator, ProofGenerator};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);
        let params = gen_params();

        let prover_options = ProverOptions {
            enable_compression: false,
            use_mock_prover: true,
            ..Default::default()
        };

        let (proof, _) = BtcBlockProofGenerator::new(params.rollup())
            .get_proof(block, &prover_options)
            .unwrap();

        let output: BlockspaceProofOutput = SP1Verifier::extract_borsh_public_output(&proof)
            .expect("Failed to extract public outputs");

        println!("{:?}", output);
    }
}
