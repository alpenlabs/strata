#[cfg(feature = "prover")]

mod test {
    use express_risc0_adapter::RiscZeroHost;
    use express_zkvm::{AggregationInput, Proof, VerifcationKey, ZKVMHost};
    use risc0_guest_builder::GUEST_CL_AGG_ELF;

    const NUM_SLOTS: usize = 4;

    fn get_prover_input() -> Vec<AggregationInput> {
        let cl_proofs: [&[u8]; NUM_SLOTS] = [
            include_bytes!("../../test-util/cl_stfs/slot-1/cl_proof_slot1_r0.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-2/cl_proof_slot2_r0.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-3/cl_proof_slot3_r0.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-4/cl_proof_slot4_r0.bin"),
        ];

        let cl_proof_vkey = include_bytes!("../../test-util/cl_stfs/slot-1/cl_vkey_slot1_r0.bin");

        let proof_inputs: Vec<AggregationInput> = cl_proofs
            .into_iter()
            .map(|proof| {
                let proof: Proof =
                    bincode::deserialize(proof).expect("Failed to deserialize proof");
                let vkey: VerifcationKey =
                    bincode::deserialize(cl_proof_vkey).expect("Failed to deserialize vk");
                AggregationInput::new(proof, vkey)
            })
            .collect();

        proof_inputs
    }

    #[test]
    fn test_cl_agg_guest_code_trace_generation() {
        let agg_inputs = get_prover_input();
        let prover = RiscZeroHost::init(GUEST_CL_AGG_ELF.into(), Default::default());

        prover
            .prove_with_aggregation(0, agg_inputs)
            .expect("Failed to generate proof");
    }
}
