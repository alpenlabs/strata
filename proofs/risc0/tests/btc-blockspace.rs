#[cfg(feature = "prover")]
mod test {
    use std::str::FromStr;

    use bitcoin::{consensus::serialize, Address};
    use btc_blockspace::logic::{BlockspaceProofOutput, ScanRuleConfig};
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use risc0_guest_builder::ALPEN_BTC_BLOCKSPACE_RISC0_PROOF_ELF;

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        let block = alpen_test_utils::bitcoin::get_btc_mainnet_block();
        let prover = RiscZeroHost::init(
            ALPEN_BTC_BLOCKSPACE_RISC0_PROOF_ELF.into(),
            Default::default(),
        );

        let scan_config = ScanRuleConfig {
            bridge_scriptbufs: vec![Address::from_str(
                "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98",
            )
            .unwrap()
            .assume_checked()
            .script_pubkey()],
            sequencer_address: "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98"
                .to_owned(),
        };
        let serialized_block = serialize(&block);

        let (proof, _) = prover
            .prove(&[scan_config.clone()], Some(&[serialized_block]))
            .expect("Failed to generate proof");

        let _output = Risc0Verifier::extract_public_output::<BlockspaceProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
