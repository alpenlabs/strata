#[cfg(feature = "prover")]
mod test {
    use strata_proofimpl_evm_ee_stf::{ELProofInput, ELProofPublicParams};
    use strata_risc0_adapter::{Risc0Verifier, RiscZeroHost, RiscZeroProofInputBuilder};
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use strata_zkvm::{ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    const ENCODED_PROVER_INPUT: &[u8] =
        include_bytes!("../../test-util/el_block_witness_input.bin");

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        let input: ELProofInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let prover = RiscZeroHost::init(GUEST_RISC0_EVM_EE_STF_ELF.into(), Default::default());

        let prover_input = RiscZeroProofInputBuilder::new()
            .write(&input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        Risc0Verifier::extract_public_output::<ELProofPublicParams>(proof.proof())
            .expect("Failed to extract public outputs");
    }
}
