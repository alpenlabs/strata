#[cfg(feature = "prover")]
mod test {
    use alpen_test_utils::bitcoin::{get_btc_chain, BtcChain};
    use bitcoin::hashes::Hash;
    use btc_blockspace::logic::BlockspaceProofOutput;
    use btc_headerchain::{
        header_verification::{get_difficulty_adjustment_height, HeaderVerificationState},
        logic::{L1BatchProofInput, L1BatchProofOutput},
    };
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use express_zkvm::{ZKVMHost, ZKVMVerifier};
    use rand::Rng;
    use risc0_guest_builder::L1_BATCH_RISC0_ELF;

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
        let prover = RiscZeroHost::init(L1_BATCH_RISC0_ELF.into(), Default::default());
        let chain = get_btc_chain();

        let h1 = get_difficulty_adjustment_height(1, chain.start);
        let r1 = rand::thread_rng().gen_range(h1..chain.end);

        let batch: Vec<BlockspaceProofOutput> = (r1..r1 + 10)
            .map(|h| BlockspaceProofOutput {
                header: chain.get_header(h),
                deposits: Vec::new(),
                forced_inclusions: Vec::new(),
                state_updates: Vec::new(),
            })
            .collect();

        let input = L1BatchProofInput {
            batch,
            state: for_block(r1, &chain),
        };

        let (proof, _) = prover
            .prove(&[input], None)
            .expect("Failed to generate proof");

        let _output = Risc0Verifier::extract_public_output::<L1BatchProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
