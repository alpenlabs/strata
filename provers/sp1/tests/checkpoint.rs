mod helpers;
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {

    use express_proofimpl_checkpoint::{
        CheckpointProofInput, CheckpointProofOutput, L2BatchProofOutput,
    };
    use express_proofimpl_l1_batch::L1BatchProofOutput;
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::GUEST_CHECKPOINT_ELF;
    use express_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};
    use num_bigint::BigUint;
    use num_traits::Num;
    use sp1_sdk::{HashableKey, MockProver, Prover};

    use crate::helpers::{
        BtcBlockProofGenerator, ClProofGenerator, ElProofGenerator, L1BatchProofGenerator,
        L2BatchProofGenerator, ProofGenerator,
    };

    #[test]
    fn test_checkpoint_proof() {
        sp1_sdk::utils::setup_logger();

        let l1_start_height = 40321;
        let l1_end_height = 40323;

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
        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&l1_batch_proof).unwrap();
        let l1_batch: L1BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
        let l1_batch_proof_agg_input = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let (l2_batch_proof, l2_batch_vk) = l2_batch_prover
            .get_proof(&(l2_start_height, l2_end_height), &prover_options)
            .unwrap();
        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&l2_batch_proof).unwrap();
        let l2_batch: L2BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
        let l2_batch_proof_agg_input = AggregationInput::new(l2_batch_proof, l2_batch_vk);

        let prover = SP1Host::init(GUEST_CHECKPOINT_ELF.into(), prover_options);

        let mock_prover = MockProver::new();
        let (_, vk) = mock_prover.setup(GUEST_CHECKPOINT_ELF);
        let vk = BigUint::from_str_radix(vk.bytes32().strip_prefix("0x").unwrap(), 16)
            .unwrap()
            .to_bytes_be();

        let checkpoint_proof_input = CheckpointProofInput {
            l1_state: l1_batch,
            l2_state: l2_batch,
            vk,
        };

        let prover_input = SP1ProofInputBuilder::new()
            .write_borsh(&checkpoint_proof_input)
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

        let output_raw = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: CheckpointProofOutput = borsh::from_slice(&output_raw).unwrap();
    }
}
