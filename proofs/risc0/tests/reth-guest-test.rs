#[cfg(feature = "prover")]
mod test {
    // use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    // use express_zkvm::{ZKVMHost, ZKVMVerifier};
    // use risc0_guest_builder::RETH_RISC0_ELF;
    // use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    // const ENCODED_PROVER_INPUT: &[u8] =
    //     include_bytes!("../../test-util/el_block_witness_input.bin");

    // #[test]
    // fn test_reth_stf_guest_code_trace_generation() {
    //     let input: ZKVMInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
    //     let prover = RiscZeroHost::init(RETH_RISC0_ELF.into(), Default::default());

    //     let (proof, _) = prover
    //         .prove(input.clone())
    //         .expect("Failed to generate proof");

    //     Risc0Verifier::extract_public_output::<ELProofPublicParams>(&proof)
    //         .expect("Failed to extract public outputs");
    // }
}
