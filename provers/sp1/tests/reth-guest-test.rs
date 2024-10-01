// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
// #[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use express_proofimpl_evm_ee_stf::{ELProofInput, ELProofPublicParams};
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use express_zkvm::{ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    const ID: u32 = 2;
    const ENCODED_PROVER_INPUT: &[u8] = include_bytes!("../../test-util/witness_2.json");

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        if cfg!(debug_assertions) {
            panic!("SP1 prover runs in release mode only");
        }

        let json_str =
            std::str::from_utf8(ENCODED_PROVER_INPUT).expect("Failed to convert bytes to string");
        let input: ELProofInput = serde_json::from_str(json_str).unwrap();
        let prover_ops = ProverOptions {
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_mock_prover: false,
        };

        let prover = SP1Host::init(GUEST_EVM_EE_STF_ELF.into(), prover_ops);

        let proof_input = SP1ProofInputBuilder::new()
            .write(&input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, vk) = prover.prove(proof_input).expect("Failed to generate proof");

        use std::{
            fs::File,
            io::{self, Write},
        };

        let file_path = "el_vkey.bin";
        let mut file = File::create(file_path).unwrap();
        file.write_all(vk.as_bytes()).unwrap();

        let file_path = format!("el_proof_{:?}.bin", ID);
        let mut file = File::create(file_path).unwrap();
        file.write_all(proof.as_bytes()).unwrap();

        SP1Verifier::extract_public_output::<ELProofPublicParams>(&proof)
            .expect("Failed to extract public outputs");
    }
}
