// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(feature = "prover")]
mod test {
    use express_sp1_adapter::{SP1Host, SP1Verifier};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use sp1_guest_builder::GUEST_RETH_STF_ELF;
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    const ENCODED_PROVER_INPUT: &[u8] =
        include_bytes!("../../test-util/el_block_witness_input.bin");

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        if cfg!(debug_assertions) {
            panic!("SP1 prover runs in release mode only");
        }

        let input: ZKVMInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let prover = SP1Host::init(GUEST_RETH_STF_ELF.into(), Default::default());

        let (proof, _) = prover
            .prove(input.clone())
            .expect("Failed to generate proof");

        SP1Verifier::extract_public_output::<ELProofPublicParams>(&proof)
            .expect("Failed to extract public outputs");
    }
}

#[cfg(feature = "prover")]
mod test_2 {
    use std::{fs::File, io::Write};

    use express_sp1_adapter::{SP1Host, SP1Verifier};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use sp1_guest_builder::GUEST_RETH_STF_ELF;
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    #[test]
    fn test_el_again() {
        let encoded_prover_input = include_bytes!("../../test-util/el_stfs/zk-input-1.json");
        let input: ZKVMInput = serde_json::from_slice(encoded_prover_input).unwrap();

        let prover = SP1Host::init(GUEST_RETH_STF_ELF.into(), Default::default());

        let (proof, vk) = prover
            .prove(input.clone())
            .expect("Failed to generate proof");

        // Serialize both into binary format
        let serialized_proof = bincode::serialize(&proof).unwrap();
        let serialized_vk = bincode::serialize(&vk).unwrap();

        let mut file_proof = File::create("proof.bin").unwrap();
        file_proof.write_all(&serialized_proof).unwrap();

        let mut file_vk = File::create("vk.bin").unwrap();
        file_vk.write_all(&serialized_vk).unwrap();
    }
}
