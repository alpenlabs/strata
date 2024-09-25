// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(feature = "prover")]
mod test {
    use alpen_test_utils::bitcoin::get_tx_filters;
    use bitcoin::params::MAINNET;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_proofimpl_l1_batch::{
        logic::{L1BatchProofInput, L1BatchProofOutput},
        mock::get_verification_state_for_block,
        pow_params::PowParams,
    };
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::{GUEST_BTC_BLOCKSPACE_ELF, GUEST_L1_BATCH_ELF};
    use express_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    #[test]
    fn test_l1_batch_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let mainnet_blocks: Vec<(u32, String)> = vec![
            (40321, "0100000045720d24eae33ade0d10397a2e02989edef834701b965a9b161e864500000000993239a44a83d5c427fd3d7902789ea1a4d66a37d5848c7477a7cf47c2b071cd7690784b5746651c3af7ca030101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c02db00ffffffff0100f2052a01000000434104c9f513361104db6a84fb6d5b364ba57a27cd19bd051239bf750d8999c6b437220df8fea6b932a248df3cad1fdebb501791e02b7b893a44718d696542ba92a0acac00000000".to_owned()),
            // (40322, "01000000fd1133cd53d00919b0bd77dd6ca512c4d552a0777cc716c00d64c60d0000000014cf92c7edbe8a75d1e328b4fec0d6143764ecbd0f5600aba9d22116bf165058e590784b5746651c1623dbe00101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c020509ffffffff0100f2052a010000004341043eb751f57bd4839a8f2922d5bf1ed15ade9b161774658fb39801f0b9da9c881f226fbe4ee0c240915f17ce5255dd499075ab49b199a7b1f898fb20cc735bc45bac00000000".to_owned()),
            // (40323, "01000000c579e586b48485b6e263b54949d07dce8660316163d915a35e44eb570000000011d2b66f9794f17393bf90237f402918b61748f41f9b5a2523c482a81a44db1f4f91784b5746651c284557020101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c024502ffffffff0100f2052a01000000434104597b934f2081e7f0d7fae03ec668a9c69a090f05d4ee7c65b804390d94266ffb90442a1889aaf78b460692a43857638520baa8319cf349b0d5f086dc4d36da8eac00000000".to_owned()),
            // (40324, "010000001f35c6ea4a54eb0ea718a9e2e9badc3383d6598ff9b6f8acfd80e52500000000a7a6fbce300cbb5c0920164d34c36d2a8bb94586e9889749962b1be9a02bbf3b9194784b5746651c0558e1140101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c029001ffffffff0100f2052a01000000434104e5d390c21b7d221e6ba15c518444c1aae43d6fb6f721c4a5f71e590288637ca2961be07ee845a795da3fd1204f52d4faa819c167062782590f08cf717475e488ac00000000".to_owned()),
        ];

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: true,
        };
        let prover = SP1Host::init(
            GUEST_BTC_BLOCKSPACE_ELF.into(),
            // Default::default(),
            prover_options,
        );

        let mut blockspace_outputs = Vec::new();
        let mut blockspace_proofs = Vec::new();

        let mut l1_batch_input_builder = SP1ProofInputBuilder::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let filters = get_tx_filters();

            let blockspace_input = SP1ProofInputBuilder::new()
                .write_borsh(&filters)
                .unwrap()
                .write_serialized(&block_bytes)
                .unwrap()
                .build()
                .unwrap();

            let (proof, vkey) = prover
                .prove(blockspace_input)
                .expect("Failed to generate proof");

            let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
                .expect("Failed to extract public outputs");
            let output: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();

            blockspace_outputs.push(output);
            blockspace_proofs.push(AggregationInput::new(proof, vkey));
        }

        let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), prover_options);
        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: get_verification_state_for_block(40321, &PowParams::from(&MAINNET)),
        };
        l1_batch_input_builder.write_borsh(&input).unwrap();

        for proof in blockspace_proofs {
            l1_batch_input_builder.write_proof(proof).unwrap();
        }

        let l1_batch_input = l1_batch_input_builder.build().unwrap();

        let (proof, _) = prover
            .prove(l1_batch_input)
            .expect("Failed to generate proof");

        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: L1BatchProofOutput = borsh::from_slice(&raw_output).unwrap();
    }
}
