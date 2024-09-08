#[cfg(feature = "prover")]

mod test {
    use express_sp1_adapter::SP1Host;
    use express_zkvm::{AggregationInput, Proof, VerifcationKey, ZKVMHost};
    use sp1_guest_builder::GUEST_CL_AGG_ELF;

    const NUM_SLOTS: usize = 4;

    fn get_prover_input() -> Vec<AggregationInput> {
        let cl_proofs: [&[u8]; NUM_SLOTS] = [
            include_bytes!("../../test-util/cl_stfs/slot-1/cl_proof_slot1_sp1.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-2/cl_proof_slot2_sp1.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-3/cl_proof_slot3_sp1.bin"),
            include_bytes!("../../test-util/cl_stfs/slot-4/cl_proof_slot4_sp1.bin"),
        ];

        let cl_proof_vkey = include_bytes!("../../test-util/cl_stfs/slot-1/cl_vkey_slot1_sp1.bin");

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
        let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), Default::default());

        prover
            .prove_with_aggregation("cl_agg", agg_inputs)
            .expect("Failed to generate proof");
    }
}
