#[cfg(feature = "prover")]
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
    use express_sp1_guest_builder::GUEST_CL_AGG_ELF;
    use express_zkvm::{
        AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
    };

    const NUM_SLOTS: usize = 2;

    fn get_prover_input() -> Vec<AggregationInput> {
        let cl_proofs: [&[u8]; NUM_SLOTS] = [
            include_bytes!("../../test-util/cl_agg/cl_proof_1.bin"),
            include_bytes!("../../test-util/cl_agg/cl_proof_2.bin"),
        ];

        let cl_proof_vkey = include_bytes!("../../test-util/cl_agg/cl_vkey.bin");

        let proof_inputs: Vec<AggregationInput> = cl_proofs
            .into_iter()
            .map(|proof| {
                let proof = Proof::new(proof.to_vec());
                let vkey = VerificationKey::new(cl_proof_vkey.to_vec());
                AggregationInput::new(proof, vkey)
            })
            .collect();

        proof_inputs
    }

    #[test]
    fn test_cl_agg_guest_code_trace_generation() {
        let agg_proof_inputs = get_prover_input();

        let prover_ops = ProverOptions {
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_mock_prover: true,
        };

        let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), prover_ops);

        let mut prover_input_builder = SP1ProofInputBuilder::new();
        prover_input_builder.write(&NUM_SLOTS).unwrap();
        for agg_proof in agg_proof_inputs {
            prover_input_builder
                .write_proof_with_vkey(agg_proof)
                .unwrap();
        }

        let prover_input = prover_input_builder.build().unwrap();
        let (proof, vk) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");
    }
}
