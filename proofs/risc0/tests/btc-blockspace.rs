#[cfg(feature = "prover")]
mod test {
    use bitcoin_blockspace::logic::{BlockspaceProofInput, BlockspaceProofOutput};
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use risc0_guest_builder::BTC_BLOCKSPACE_RISC0_ELF;

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        let block = alpen_test_utils::bitcoin::get_btc_mainnet_block();
        let prover = RiscZeroHost::init(BTC_BLOCKSPACE_RISC0_ELF.into(), Default::default());

        let input = BlockspaceProofInput {
            block,
            bridge_address: "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98"
                .to_owned(),
        };

        let (proof, _) = prover.prove(input).expect("Failed to generate proof");

        let _output = Risc0Verifier::extract_public_output::<BlockspaceProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
