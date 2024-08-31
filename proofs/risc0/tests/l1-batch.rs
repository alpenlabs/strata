#[cfg(feature = "prover")]
mod test {
    use std::str::FromStr;

    use alpen_test_utils::bitcoin::{get_btc_chain, BtcChain};
    use bitcoin::{consensus::deserialize, hashes::Hash, Address, Block};
    use btc_blockspace::logic::{BlockspaceProofInput, BlockspaceProofOutput, ScanRuleConfig};
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use express_zkvm::{ProverOptions, ZKVMHost, ZKVMVerifier};
    use l1_batch::{
        header_verification::{get_difficulty_adjustment_height, HeaderVerificationState},
        logic::{L1BatchProofInput, L1BatchProofOutput},
    };
    use risc0_guest_builder::{ALPEN_BTC_BLOCKSPACE_RISC0_PROOF_ELF, L1_BATCH_RISC0_ELF};
    use risc0_zkvm::Receipt;

    fn for_block(block_height: u32, chain: &BtcChain) -> HeaderVerificationState {
        // Get the first difficulty adjustment block after `chain.start`
        let h1 = get_difficulty_adjustment_height(1, chain.start);
        assert!(
            block_height > h1 && block_height < chain.end,
            "not enough info in the chain"
        );

        // Get the difficulty adjustment block just before `block_height`
        let h1 = get_difficulty_adjustment_height(0, block_height);

        // Consider the block before `block_height` to be the last verified block
        let vh = block_height - 1; // verified_height

        // Fetch the previous timestamps of block from `vh`
        // This fetches timestamps of `vh`, `vh-1`, `vh-2`, ...
        let recent_block_timestamps: [u32; 11] =
            chain.get_last_timestamps(vh, 11).try_into().unwrap();

        HeaderVerificationState {
            last_verified_block_num: vh,
            last_verified_block_hash: chain
                .get_header(vh)
                .block_hash()
                .as_raw_hash()
                .to_byte_array()
                .into(),
            next_block_target: chain
                .get_header(vh)
                .target()
                .to_compact_lossy()
                .to_consensus(),
            interval_start_timestamp: chain.get_header(h1).time,
            total_accumulated_pow: 0f64,
            last_11_blocks_timestamps: recent_block_timestamps,
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
            enable_compression: false,
            use_mock_prover: false,
            stark_to_snark_conversion: false,
        };
        // let prover = RiscZeroHost::init(ALPEN_BTC_BLOCKSPACE_RISC0_PROOF_ELF.into(),
        // Default::default());
        let prover =
            RiscZeroHost::init(ALPEN_BTC_BLOCKSPACE_RISC0_PROOF_ELF.into(), prover_options);

        let mut blockspace_proofs = Vec::new();
        let mut blockspace_outputs = Vec::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let block: Block = deserialize(&block_bytes).unwrap();
            let scan_config = ScanRuleConfig {
                bridge_scriptbufs: vec![Address::from_str(
                    "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98",
                )
                .unwrap()
                .assume_checked()
                .script_pubkey()],
            };
            let input = BlockspaceProofInput { block, scan_config };

            let (proof, _) = prover
                .prove(&[input], None)
                .expect("Failed to generate proof");

            let output = Risc0Verifier::extract_public_output::<BlockspaceProofOutput>(&proof)
                .expect("Failed to extract public outputs");

            let receipt: Receipt =
                bincode::deserialize(proof.as_bytes()).expect("bincode deserialize must not fail");

            blockspace_proofs.push(receipt);
            blockspace_outputs.push(output);
        }

        let prover_options = ProverOptions {
            enable_compression: true,
            use_mock_prover: false,
            stark_to_snark_conversion: false,
        };
        // let prover = RiscZeroHost::init(L1_BATCH_RISC0_ELF.into(), Default::default());
        let prover = RiscZeroHost::init(L1_BATCH_RISC0_ELF.into(), prover_options);
        let chain = get_btc_chain();
        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: for_block(40321, &chain),
        };

        let (proof, _) = prover
            .prove_with_assumptions(input, blockspace_proofs)
            .expect("Failed to generate proof");

        let _output = Risc0Verifier::extract_public_output::<L1BatchProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
