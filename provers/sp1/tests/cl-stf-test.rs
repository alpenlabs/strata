mod helpers;

#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use std::path::Path;

    use alpen_express_state::{block::L2Block, chain_state::ChainState};
    use express_proofimpl_checkpoint::L2BatchProofOutput;
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::GUEST_CL_STF_ELF;
    use express_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    use super::helpers::get_el_block_proof;

    fn get_prover_input() -> (ChainState, L2Block, AggregationInput) {
        let el_witness_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-util/el/witness_2.json");
        let (el_proof, vk) = get_el_block_proof(&el_witness_path).unwrap();
        let cl_witness: &[u8] = include_bytes!("../../test-util/cl/cl_witness_2.bin");
        let (prev_state, block): (ChainState, L2Block) = borsh::from_slice(cl_witness).unwrap();
        let agg_input = AggregationInput::new(el_proof, vk);
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
            .write_proof(agg_input)
            .unwrap()
            .write(&input_ser)
            .unwrap()
            .build()
            .unwrap();

        println!("Generating the CL proof");
        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        println!("Generated the proof...");
        let cl_stf_pp_ser = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");

        let _cl_stf_pp: L2BatchProofOutput = borsh::from_slice(&cl_stf_pp_ser).unwrap();
    }
}
