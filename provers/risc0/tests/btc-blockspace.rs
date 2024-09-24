#[cfg(feature = "prover")]
mod test {
    use alpen_test_utils::bitcoin::{get_btc_mainnet_block, get_tx_filters};
    use bitcoin::consensus::serialize;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost, RiscZeroProofInputBuilder};
    use express_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use express_zkvm::{ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        let block = get_btc_mainnet_block();
        let filters = get_tx_filters();
        let prover = RiscZeroHost::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF.into(), Default::default());

        let serialized_block = serialize(&block);

        let input = RiscZeroProofInputBuilder::new()
            .write_borsh(&filters)
            .unwrap()
            .write_serialized(&serialized_block)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover.prove(input).expect("Failed to generate proof");

        // TODO: add `extract_public_output_borsh` function to Verifier
        let raw_output = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");

        let _: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
