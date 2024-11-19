#[cfg(feature = "prover")]
mod test {
    use bitcoin::consensus::serialize;
    use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder, Risc0Verifier};
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use strata_test_utils::bitcoin::{get_btc_mainnet_block, get_tx_filters};
    use strata_zkvm::{ZkVmHost, ZkVmInputBuilder, ZkVmVerifier};

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        let block = get_btc_mainnet_block();
        let filters = get_tx_filters();
        let prover = Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF.into(), Default::default());

        let serialized_block = serialize(&block);

        let input = Risc0ProofInputBuilder::new()
            .write_borsh(&filters)
            .unwrap()
            .write_buf(&serialized_block)
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
