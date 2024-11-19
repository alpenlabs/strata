#[cfg(feature = "prover")]
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder, Risc0Verifier};
    use strata_risc0_guest_builder::GUEST_RISC0_CL_STF_ELF;
    use strata_state::{block::L2Block, chain_state::ChainState};
    use strata_zkvm::{ZkVmHost, ZkVmInputBuilder, ZkVmVerifier};

    fn get_prover_input() -> (ChainState, L2Block) {
        let prev_state_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-1/prev_chstate.borsh");
        let new_block_data: &[u8] =
            include_bytes!("../../test-util/cl_stfs/slot-1/final_block.borsh");

        let prev_state: ChainState = borsh::from_slice(prev_state_data).unwrap();
        let block: L2Block = borsh::from_slice(new_block_data).unwrap();

        (prev_state, block)
    }

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        let input = get_prover_input();

        let prover = Risc0Host::init(GUEST_RISC0_CL_STF_ELF.into(), Default::default());

        // TODO: handle this properly
        let input = Risc0ProofInputBuilder::new()
            .write_borsh(&input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover.prove(input).expect("Failed to generate proof");

        let new_state_ser = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");

        let _new_state: ChainState = borsh::from_slice(&new_state_ser).unwrap();
    }
}
