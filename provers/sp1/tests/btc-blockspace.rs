mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_test_utils::bitcoin::get_btc_chain;
    use strata_zkvm::{ProverOptions, ZkVmVerifier};

    use crate::helpers::{BtcBlockProofGenerator, ProofGenerator};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let prover_options = ProverOptions {
            enable_compression: true,
            use_mock_prover: true,
            ..Default::default()
        };

        let (proof, _) = BtcBlockProofGenerator::new()
            .get_proof(block, &prover_options)
            .unwrap();

        let _: BlockspaceProofOutput = SP1Verifier::extract_borsh_public_output(&proof)
            .expect("Failed to extract public outputs");
    }
}
