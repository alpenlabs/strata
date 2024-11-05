mod helpers;
// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {

    use strata_test_utils::l2::gen_params;
    use strata_zkvm::{AggregationInput, ProverOptions, ZkVmHost, ZkVmInputBuilder, ZkVmVerifier};

    use crate::helpers::{
        BtcBlockProofGenerator, CheckpointBatchInfo, CheckpointProofGenerator, ClProofGenerator,
        ElProofGenerator, L1BatchProofGenerator, L2BatchProofGenerator, ProofGenerator,
    };

    #[test]
    fn test_checkpoint_proof() {
        sp1_sdk::utils::setup_logger();

        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 2;

        let l2_start_height = 1;
        let l2_end_height = 3;

        let btc_prover = BtcBlockProofGenerator::new();
        let l1_batch_prover = L1BatchProofGenerator::new(btc_prover);
        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let l2_batch_prover = L2BatchProofGenerator::new(cl_prover);
        let checkpoint_prover = CheckpointProofGenerator::new(l1_batch_prover, l2_batch_prover);

        let prover_input = CheckpointBatchInfo {
            l1_range: (l1_start_height.into(), l1_end_height.into()),
            l2_range: (l2_start_height, l2_end_height),
        };

        let _ = checkpoint_prover
            .get_proof(&prover_input)
            .expect("Failed to generate proof");
    }
}
