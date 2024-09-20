// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(feature = "prover")]
mod test {
    use std::str::FromStr;

    use bitcoin::Address;
    use strata_proofimpl_btc_blockspace::logic::{BlockspaceProofOutput, ScanRuleConfig};
    use strata_sp1_adapter::{SP1Host, SP1Verifier};
    use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
    use strata_zkvm::{ProverInput, ZKVMHost, ZKVMVerifier};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        if cfg!(debug_assertions) {
            panic!("SP1 prover runs in release mode only");
        }

        let block = test_utils::bitcoin::get_btc_mainnet_block();
        let scan_config = ScanRuleConfig {
            bridge_scriptbufs: vec![Address::from_str(
                "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98",
            )
            .unwrap()
            .assume_checked()
            .script_pubkey()],
        };
        let serialized_block = bitcoin::consensus::serialize(&block);

        let prover = SP1Host::init(GUEST_BTC_BLOCKSPACE_ELF.into(), Default::default());

        let mut prover_input = ProverInput::new();
        prover_input.write(scan_config.clone());
        prover_input.write_serialized(serialized_block);

        let (proof, _) = prover
            .prove(&prover_input)
            .expect("Failed to generate proof");

        SP1Verifier::extract_public_output::<BlockspaceProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
