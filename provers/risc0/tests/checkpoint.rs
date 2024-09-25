#[cfg(feature = "prover")]
mod test {
    use alpen_test_utils::{bitcoin::get_tx_filters, l2::get_genesis_chainstate};
    use bitcoin::params::MAINNET;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_proofimpl_checkpoint::{
        CheckpointProofOutput, HashedCheckpointState, L2BatchProofOutput,
    };
    use express_proofimpl_l1_batch::{
        logic::{L1BatchProofInput, L1BatchProofOutput},
        mock::get_verification_state_for_block,
        pow_params::PowParams,
    };
    use express_risc0_adapter::{Risc0Verifier, RiscZeroHost, RiscZeroProofInputBuilder};
    use express_risc0_guest_builder::{
        GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_BTC_BLOCKSPACE_ID, GUEST_RISC0_CHECKPOINT_ELF,
        GUEST_RISC0_L1_BATCH_ELF, GUEST_RISC0_L1_BATCH_ID,
    };
    use express_zkvm::{
        AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
        ZKVMVerifier,
    };

    // TODO: handle this repeat
    fn get_l1_batch_output_and_proof() -> (L1BatchProofOutput, Proof) {
        let mainnet_blocks: Vec<(u32, String)> = vec![
            (40321, "0100000045720d24eae33ade0d10397a2e02989edef834701b965a9b161e864500000000993239a44a83d5c427fd3d7902789ea1a4d66a37d5848c7477a7cf47c2b071cd7690784b5746651c3af7ca030101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c02db00ffffffff0100f2052a01000000434104c9f513361104db6a84fb6d5b364ba57a27cd19bd051239bf750d8999c6b437220df8fea6b932a248df3cad1fdebb501791e02b7b893a44718d696542ba92a0acac00000000".to_owned()),
        ];

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: false,
        };
        let prover = RiscZeroHost::init(
            GUEST_RISC0_BTC_BLOCKSPACE_ELF.into(),
            // Default::default(),
            prover_options,
        );

        let btc_blockspace_elf_id: Vec<u8> = GUEST_RISC0_BTC_BLOCKSPACE_ID
            .iter()
            .flat_map(|&x| x.to_le_bytes())
            .collect();

        let mut blockspace_outputs = Vec::new();
        let mut prover_input = RiscZeroProofInputBuilder::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let filters = get_tx_filters();
            let inner_prover_input = RiscZeroProofInputBuilder::new()
                .write_borsh(&filters)
                .unwrap()
                .write_serialized(&block_bytes)
                .unwrap()
                .build()
                .unwrap();

            let (proof, _) = prover
                .prove(inner_prover_input)
                .expect("Failed to generate proof");

            let raw_output = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
                .expect("Failed to extract public outputs");
            let output: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();

            prover_input
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
            state: get_verification_state_for_block(40321, &PowParams::from(&MAINNET)),
        };

        let prover_input = prover_input.write_borsh(&input).unwrap().build().unwrap();
        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let output_raw = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let output: L1BatchProofOutput = borsh::from_slice(&output_raw).unwrap();

        (output, proof)
    }

    // fn get_l1_batch_output() -> L1BatchProofOutput {
    //     let params = PowParams::from(&MAINNET);
    //     L1BatchProofOutput {
    //         deposits: Vec::new(),
    //         forced_inclusions: Vec::new(),
    //         state_update: None,
    //         initial_state: get_verification_state_for_block(40_320, &params),
    //         final_state: get_verification_state_for_block(40_321, &params),
    //     }
    // }

    fn get_l2_batch_output() -> L2BatchProofOutput {
        L2BatchProofOutput {
            deposits: Vec::new(),
            initial_state: get_genesis_chainstate(),
            final_state: get_genesis_chainstate(),
        }
    }

    #[test]
    fn test_checkpoint_proof() {
        let (l1_batch, l1_batch_proof) = get_l1_batch_output_and_proof();
        // let l1_batch = get_l1_batch_output();
        let l2_batch = get_l2_batch_output();

        let genesis = HashedCheckpointState {
            l1_state: l1_batch.initial_state.hash().unwrap(),
            l2_state: l2_batch.initial_state.compute_state_root(),
        };

        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: false,
        };
        let prover = RiscZeroHost::init(GUEST_RISC0_CHECKPOINT_ELF.into(), prover_options);

        let l1_batch_image_id: Vec<u8> = GUEST_RISC0_L1_BATCH_ID
            .iter()
            .flat_map(|&x| x.to_le_bytes())
            .collect();
        let l1_batch_proof_input = AggregationInput::new(
            l1_batch_proof,
            VerificationKey::new(l1_batch_image_id.clone()),
        );

        let prover_input = RiscZeroProofInputBuilder::new()
            .write_borsh(&l1_batch)
            .unwrap()
            .write_serialized(&borsh::to_vec(&l2_batch).unwrap())
            .unwrap()
            .write_serialized(&borsh::to_vec(&genesis).unwrap())
            .unwrap()
            .write_proof(l1_batch_proof_input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let _output = Risc0Verifier::extract_public_output::<CheckpointProofOutput>(&proof)
            .expect("Failed to extract public outputs");
    }
}
