mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_test_utils::l2::gen_params;
    use strata_zkvm::{ProverOptions, ZkVmVerifier};

    use crate::helpers::{BtcBlockProofGenerator, L1BatchProofGenerator, ProofGenerator};

    #[test]
    fn test_l1_batch_code_trace_generation() {
        sp1_sdk::utils::setup_logger();

        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 2;

        let btc_proof_generator = BtcBlockProofGenerator::new();
        let _ = L1BatchProofGenerator::new(btc_proof_generator)
            .get_proof(&(l1_start_height, l1_end_height))
            .unwrap();
    }
}
