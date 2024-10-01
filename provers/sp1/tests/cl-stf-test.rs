#[cfg(feature = "prover")]

mod test {
    use alpen_express_state::{block::L2Block, chain_state::ChainState};
    use express_proofimpl_cl_stf::CLProofPublicParams;
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::GUEST_CL_STF_ELF;
    use express_zkvm::{
        AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
        ZKVMVerifier,
    };

    fn get_prover_input() -> (ChainState, L2Block, AggregationInput) {
        let el_proof: &[u8] = include_bytes!("../../test-util/cl/el_proof_2.bin");
        let cl_witness: &[u8] = include_bytes!("../../test-util/cl/cl_witness_2");
        let el_vkey: &[u8] = include_bytes!("../../test-util/cl/el_vkey.bin");
        let (prev_state, block): (ChainState, L2Block) = borsh::from_slice(cl_witness).unwrap();

        let proof = Proof::new(el_proof.to_vec());
        let vk = VerificationKey::new(el_vkey.to_vec());
        let agg_input = AggregationInput::new(proof, vk);

        (prev_state, block, agg_input)
    }

    #[test]
    fn test_cl_stf_guest_code_trace_generation() {
        // let input: ELProofInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let (prev_state, block, agg_input) = get_prover_input();
        let input = (prev_state, block);
        let input_ser = borsh::to_vec(&input).unwrap();

        let prover_ops = ProverOptions {
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_mock_prover: true,
        };
        let prover = SP1Host::init(GUEST_CL_STF_ELF.into(), prover_ops);

        let prover_input = SP1ProofInputBuilder::new()
            .write_proof_with_vkey(agg_input)
            .unwrap()
            .write(&input_ser)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let _new_state_ser = SP1Verifier::extract_public_output::<CLProofPublicParams>(&proof)
            .expect("Failed to extract public outputs");
    }
}
