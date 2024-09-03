#[cfg(feature = "prover")]

mod test {
    use alpen_express_state::{block::L2Block, chain_state::ChainState};
    use express_sp1_adapter::{SP1Host, SP1Verifier};
    use express_zkvm::{Proof, VerifcationKey, ZKVMHost, ZKVMVerifier};
    use sp1_guest_builder::GUEST_CL_STF_ELF;

    fn get_prover_input() -> (ChainState, L2Block, Proof, VerifcationKey) {
        let prev_state_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-1/prev_chstate.borsh");
        let new_block_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-1/final_block.borsh");

        let prev_el_proof_ser: &[u8] = include_bytes!("../../test-util/el_stfs/slot-1/proof.bin");
        let el_proof_vk_ser: &[u8] = include_bytes!("../../test-util/el_stfs/slot-1/vk.bin");

        let prev_state: ChainState = borsh::from_slice(prev_state_data).unwrap();
        let block: L2Block = borsh::from_slice(new_block_data).unwrap();

        let prev_el_proof: Proof = bincode::deserialize(prev_el_proof_ser).unwrap();
        let el_proof_vk: VerifcationKey = bincode::deserialize(el_proof_vk_ser).unwrap();

        (prev_state, block, prev_el_proof, el_proof_vk)
    }

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        // let input: ZKVMInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let (prev_state, block, prev_el_proof, el_proof_vk) = get_prover_input();

        let input_ser = borsh::to_vec(&(prev_state, block)).unwrap();

        let prover = SP1Host::init(GUEST_CL_STF_ELF.into(), Default::default());

        let (proof, _) = prover
            .prove_v2((el_proof_vk, prev_el_proof, input_ser))
            .expect("Failed to generate proof");

        // let new_state_ser = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
        //     .expect("Failed to extract public outputs");

        // let _new_state: ChainState = borsh::from_slice(&new_state_ser).unwrap();
    }
}
