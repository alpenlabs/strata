// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(feature = "prover")]
mod test {
    use bitcoin_blockspace::logic::{BlockspaceProofInput, BlockspaceProofOutput, ScanParams};
    use express_sp1_adapter::{SP1Host, SP1Verifier};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        if cfg!(debug_assertions) {
            panic!("SP1 prover runs in release mode only");
        }

        let block = alpen_test_utils::bitcoin::get_btc_mainnet_block();
        let scan_params = ScanParams {
            bridge_address: "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98"
                .to_owned(),
            sequencer_address: "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98"
                .to_owned(),
        };
        // let serialized_block = serialize(&block);
        let input = BlockspaceProofInput { block, scan_params };

        let prover = SP1Host::init(GUEST_BTC_BLOCKSPACE_ELF.into(), Default::default());

        let (proof, _) = prover
            .prove(&[input], None)
            .expect("Failed to generate proof");

        SP1Verifier::extract_public_output::<BlockspaceProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
