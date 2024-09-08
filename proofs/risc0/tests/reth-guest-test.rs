#[cfg(feature = "prover")]
mod test {

    use express_risc0_adapter::RiscZeroHost;
    use express_zkvm::ZKVMHost;
    use risc0_guest_builder::RETH_RISC0_ELF;
    use zkvm_primitives::ZKVMInput;

    const ENCODED_PROVER_INPUT: &[u8] =
        include_bytes!("../../test-util/el_stfs/slot-1/zk-input-1.json");

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        let input: ZKVMInput = serde_json::from_slice(ENCODED_PROVER_INPUT).unwrap();
        let prover = RiscZeroHost::init(RETH_RISC0_ELF.into(), Default::default());

        prover
            .prove(input.clone())
            .expect("Failed to generate proof");
    }
}
