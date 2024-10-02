mod helpers;
#[cfg(feature = "prover")]
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::GUEST_CL_AGG_ELF;
    use express_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    use super::helpers::get_cl_stf_proof;

    const NUM_SLOTS: usize = 2;

    fn get_prover_input() -> Vec<AggregationInput> {
        let mut proof_inputs: Vec<AggregationInput> = Vec::new();
        for block in 1..=NUM_SLOTS {
            let (proof, vk) = get_cl_stf_proof(block as u32).unwrap();
            proof_inputs.push(AggregationInput::new(proof, vk));
        }

        proof_inputs
    }

    #[test]
    fn test_cl_agg_guest_code_ip_generation() {
        let agg_proof_inputs = get_prover_input();
        for agg in agg_proof_inputs.iter() {
            SP1Verifier::verify(agg.vk(), agg.proof()).unwrap();
        }
        println!("got the agg proof {:?}", agg_proof_inputs.len());
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
            prover_input_builder.write_proof(agg_proof).unwrap();
        }

        let prover_input = prover_input_builder.build().unwrap();
        let (_proof, _vk) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");
    }
}
