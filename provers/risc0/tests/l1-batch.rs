#[cfg(feature = "prover")]
mod test {
    use alpen_test_utils::bitcoin::{get_btc_chain, get_tx_filters};
    use bitcoin::params::MAINNET;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_proofimpl_l1_batch::{
        header_verification::HeaderVerificationState,
        logic::{L1BatchProofInput, L1BatchProofOutput},
        timestamp_store::TimestampStore,
    };
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost, RiscZeroProofInputBuilder};
    use express_risc0_guest_builder::{
        GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_BTC_BLOCKSPACE_ID, GUEST_RISC0_L1_BATCH_ELF,
    };
    use express_zkvm::{
        AggregationInput, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier,
    };

    fn get_header_verification_state(height: u32) -> HeaderVerificationState {
        let chain = get_btc_chain(MAINNET.clone());
        let (
            last_verified_block_hash,
            next_block_target,
            initial_timestamps,
            interval_start_timestamp,
        ) = chain.get_header_verification_info(height);
        let last_11_blocks_timestamps = TimestampStore::new(initial_timestamps);

        HeaderVerificationState {
            last_verified_block_num: height - 1,
            last_verified_block_hash,
            next_block_target,
            interval_start_timestamp,
            total_accumulated_pow: 0f64,
            last_11_blocks_timestamps,
        }
    }

    #[test]
    fn test_l1_batch_code_trace_generation() {
        let mainnet_blocks: Vec<(u32, String)> = vec![
            (40321, "0100000045720d24eae33ade0d10397a2e02989edef834701b965a9b161e864500000000993239a44a83d5c427fd3d7902789ea1a4d66a37d5848c7477a7cf47c2b071cd7690784b5746651c3af7ca030101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c02db00ffffffff0100f2052a01000000434104c9f513361104db6a84fb6d5b364ba57a27cd19bd051239bf750d8999c6b437220df8fea6b932a248df3cad1fdebb501791e02b7b893a44718d696542ba92a0acac00000000".to_owned()),
            (40322, "01000000fd1133cd53d00919b0bd77dd6ca512c4d552a0777cc716c00d64c60d0000000014cf92c7edbe8a75d1e328b4fec0d6143764ecbd0f5600aba9d22116bf165058e590784b5746651c1623dbe00101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c020509ffffffff0100f2052a010000004341043eb751f57bd4839a8f2922d5bf1ed15ade9b161774658fb39801f0b9da9c881f226fbe4ee0c240915f17ce5255dd499075ab49b199a7b1f898fb20cc735bc45bac00000000".to_owned()),
            (40323, "01000000c579e586b48485b6e263b54949d07dce8660316163d915a35e44eb570000000011d2b66f9794f17393bf90237f402918b61748f41f9b5a2523c482a81a44db1f4f91784b5746651c284557020101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c024502ffffffff0100f2052a01000000434104597b934f2081e7f0d7fae03ec668a9c69a090f05d4ee7c65b804390d94266ffb90442a1889aaf78b460692a43857638520baa8319cf349b0d5f086dc4d36da8eac00000000".to_owned()),
            (40324, "010000001f35c6ea4a54eb0ea718a9e2e9badc3383d6598ff9b6f8acfd80e52500000000a7a6fbce300cbb5c0920164d34c36d2a8bb94586e9889749962b1be9a02bbf3b9194784b5746651c0558e1140101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c029001ffffffff0100f2052a01000000434104e5d390c21b7d221e6ba15c518444c1aae43d6fb6f721c4a5f71e590288637ca2961be07ee845a795da3fd1204f52d4faa819c167062782590f08cf717475e488ac00000000".to_owned()),
        ];

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: false,
        };
        let prover = RiscZeroHost::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF.into(), prover_options);

        let btc_blockspace_elf_id: Vec<u8> = GUEST_RISC0_BTC_BLOCKSPACE_ID
            .iter()
            .flat_map(|&x| x.to_le_bytes())
            .collect();

        let mut blockspace_outputs = Vec::new();

        let mut l1_batch_input_builder = RiscZeroProofInputBuilder::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let filters = get_tx_filters();
            let blockspace_input = RiscZeroProofInputBuilder::new()
                .write_borsh(&filters)
                .unwrap()
                .write_serialized(&block_bytes)
                .unwrap()
                .build()
                .unwrap();

            let (proof, _) = prover
                .prove(blockspace_input)
                .expect("Failed to generate proof");

            let output_raw = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
                .expect("Failed to extract public outputs");
            let output: BlockspaceProofOutput = borsh::from_slice(&output_raw).unwrap();

            l1_batch_input_builder
                .write_proof(AggregationInput::new(
                    proof,
                    VerificationKey::new(btc_blockspace_elf_id.clone()),
                ))
                .unwrap();
            blockspace_outputs.push(output);
        }

        let prover = RiscZeroHost::init(GUEST_RISC0_L1_BATCH_ELF.into(), prover_options);
        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: get_header_verification_state(40321),
        };

        l1_batch_input_builder.write_borsh(&input).unwrap();
        let l1_batch_input = l1_batch_input_builder.build().unwrap();
        let (proof, _) = prover
            .prove(l1_batch_input)
            .expect("Failed to generate proof");

        let output_raw = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: L1BatchProofOutput = borsh::from_slice(&output_raw).unwrap();
    }
}
