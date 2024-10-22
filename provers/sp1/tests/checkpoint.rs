mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {

    use strata_sp1_adapter::SP1Verifier;
    use strata_state::batch::CheckpointProofOutput;
    use strata_test_utils::l2::gen_params;
    use strata_zkvm::{ProverOptions, ZKVMVerifier};

    use crate::helpers::{
        self, BtcBlockProofGenerator, CheckpointProofGenerator, ClProofGenerator, ElProofGenerator,
        L1BatchProofGenerator, L2BatchProofGenerator, ProofGenerator,
    };

    fn get_checkpoint_prover_and_input(
    ) -> anyhow::Result<(CheckpointProofGenerator, helpers::CheckpointBatchInfo)> {
        let params = gen_params();
        let rollup_params = params.rollup();

        let l1_range = (
            rollup_params.genesis_l1_height + 1,
            rollup_params.genesis_l1_height + 3,
        );
        let l2_range = (1, 3);

        let btc_prover = BtcBlockProofGenerator::new();
        let l1_batch_prover = L1BatchProofGenerator::new(btc_prover);
        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let l2_batch_prover = L2BatchProofGenerator::new(cl_prover);
        let checkpoint_prover = CheckpointProofGenerator::new(l1_batch_prover, l2_batch_prover);

        let checkpoint_info = helpers::CheckpointBatchInfo { l1_range, l2_range };

        Ok((checkpoint_prover, checkpoint_info))
    }

    #[test]
    fn test_checkpoint_proof() {
        sp1_sdk::utils::setup_logger();

        let prover_options = ProverOptions {
            use_mock_prover: false,
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        };

        let (prover, input) =
            get_checkpoint_prover_and_input().expect("Failed to get checkpoint input");

        let (proof, _) = prover
            .get_proof(&input, &prover_options)
            .expect("Failed to generate proof");

        let _output: CheckpointProofOutput =
            SP1Verifier::extract_borsh_public_output(proof.proof())
                .expect("Failed to extract public outputs");
    }

    #[test]
    fn test_checkpoint_proof_simulation() {
        sp1_sdk::utils::setup_logger();

        let (executor, input) =
            get_checkpoint_prover_and_input().expect("Failed to get checkpoint input");

        executor.simulate(&input).expect("Failed to simulate");
    }
}
