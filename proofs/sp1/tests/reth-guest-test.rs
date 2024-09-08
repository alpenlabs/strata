// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(feature = "prover")]
mod test {
    use express_sp1_adapter::{SP1Host, SP1Verifier};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use sp1_guest_builder::GUEST_RETH_STF_ELF;
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    const ENCODED_PROVER_INPUT: &[u8] =
        include_bytes!("../../test-util/el_stfs/slot-1/zk-input-1.json");

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

    use express_sp1_adapter::SP1Host;
    use express_zkvm::ZKVMHost;
    use sp1_guest_builder::GUEST_RETH_STF_ELF;
    use zkvm_primitives::ZKVMInput;

    #[test]
    fn test_el_again() {
        const ENCODED_PROVER_INPUT: &[u8] =
            include_bytes!("../../test-util/el_stfs/slot-4/zk-input-4.json");

        let input: ZKVMInput = serde_json::from_slice(ENCODED_PROVER_INPUT).unwrap();
        let prover = SP1Host::init(GUEST_RETH_STF_ELF.into(), Default::default());

        prover
            .prove(input.clone())
            .expect("Failed to genexrate proof");
    }
}
