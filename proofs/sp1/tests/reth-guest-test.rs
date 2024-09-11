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
            .prove(&[input.clone()], None, None)
            .expect("Failed to generate proof");

        SP1Verifier::extract_public_output::<ELProofPublicParams>(&proof)
            .expect("Failed to extract public outputs");
    }
}
