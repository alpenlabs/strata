#[cfg(feature = "prover")]
mod test {
    use std::{fs::File, io::Write};

    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use risc0_guest_builder::RETH_RISC0_ELF;
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    // const ENCODED_PROVER_INPUT: &[u8] =
    // include_bytes!("../../test-util/el_block_witness_input.bin");
    const ENCODED_PROVER_INPUT: &[u8] =
        include_bytes!("../../test-util/el_stfs/slot-1/zk-input-1.json");

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        let input: ZKVMInput = serde_json::from_slice(ENCODED_PROVER_INPUT).unwrap();
        let prover = RiscZeroHost::init(RETH_RISC0_ELF.into(), Default::default());

        let (proof, vk) = prover
            .prove(input.clone())
            .expect("Failed to generate proof");

        Risc0Verifier::extract_public_output::<ELProofPublicParams>(&proof)
            .expect("Failed to extract public outputs");

        let proof_ser = bincode::serialize(&proof).unwrap();
        let vkey_ser = bincode::serialize(&vk).unwrap();

        let mut proof_file = File::create("el_proof_slot4.bin").unwrap();
        proof_file.write_all(&proof_ser).unwrap();

        let mut vk_file = File::create("el_vkey_slot4.bin").unwrap();
        vk_file.write_all(&vkey_ser).unwrap();
    }
}
