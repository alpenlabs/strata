#[cfg(feature = "prover")]
mod test {
    use alpen_express_state::{block::L2Block, chain_state::ChainState};
    use express_sp1_adapter::SP1Host;
    use express_zkvm::{AggregationInput, Proof, VerifcationKey, ZKVMHost};
    use sp1_guest_builder::GUEST_CL_STF_ELF;

    fn get_prover_input() -> (ChainState, L2Block, Proof, VerifcationKey) {
        let prev_state_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-3/prev_chstate.borsh");
        let new_block_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-3/final_block.borsh");

        let prev_el_proof_ser: &[u8] =
            include_bytes!("../../test-util/el_stfs/slot-3/el_proof_slot3_sp1.bin");
        let el_proof_vk_ser: &[u8] =
            include_bytes!("../../test-util/el_stfs/slot-3/el_vkey_slot3_sp1.bin");

        let prev_state: ChainState = borsh::from_slice(prev_state_data).unwrap();
        let block: L2Block = borsh::from_slice(new_block_data).unwrap();

        let prev_el_proof: Proof = bincode::deserialize(prev_el_proof_ser).unwrap();
        let el_proof_vk: VerifcationKey = bincode::deserialize(el_proof_vk_ser).unwrap();

        (prev_state, block, prev_el_proof, el_proof_vk)
    }

    #[test]
    fn test_cl_stf_guest_code_trace_generation() {
        let (prev_state, block, prev_el_proof, el_proof_vk) = get_prover_input();

        let input_ser = borsh::to_vec(&(prev_state, block)).unwrap();

        let prover = SP1Host::init(GUEST_CL_STF_ELF.into(), Default::default());
        let proof_agg_input = AggregationInput::new(prev_el_proof, el_proof_vk);

        prover
            .prove_with_aggregation(input_ser, vec![proof_agg_input])
            .expect("Failed to generate proof");
    }
}
