mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {

    use strata_proofimpl_checkpoint::CheckpointProofOutput;
    use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use strata_sp1_guest_builder::GUEST_CHECKPOINT_ELF;
    use strata_test_utils::l2::gen_params;
    use strata_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    use crate::helpers::{
        BtcBlockProofGenerator, ClProofGenerator, ElProofGenerator, L1BatchProofGenerator,
        L2BatchProofGenerator, ProofGenerator,
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

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: true,
        };

        let (l1_batch_proof, l1_batch_vk) = l1_batch_prover
            .get_proof(&(l1_start_height, l1_end_height), &prover_options)
            .unwrap();
        let l1_batch_proof_agg_input = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let (l2_batch_proof, l2_batch_vk) = l2_batch_prover
            .get_proof(&(l2_start_height, l2_end_height), &prover_options)
            .unwrap();
        let l2_batch_proof_agg_input = AggregationInput::new(l2_batch_proof, l2_batch_vk);

        let prover = SP1Host::init(GUEST_CHECKPOINT_ELF.into(), prover_options);

        let prover_input = SP1ProofInputBuilder::new()
            .write(&rollup_params)
            .unwrap()
            .write_proof(l1_batch_proof_agg_input)
            .unwrap()
            .write_proof(l2_batch_proof_agg_input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let _output: CheckpointProofOutput = SP1Verifier::extract_borsh_public_output(&proof)
            .expect("Failed to extract public outputs");
    }
}
