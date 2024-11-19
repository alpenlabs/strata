#[cfg(feature = "prover")]
mod test {
    use bitcoin::params::MAINNET;
    use strata_primitives::buf::Buf32;
    use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use strata_proofimpl_cl_stf::{ChainStateSnapshot, L2BatchProofOutput};
    use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProofOutput};
    use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder, Risc0Verifier};
    use strata_risc0_guest_builder::{
        GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_BTC_BLOCKSPACE_ID, GUEST_RISC0_CHECKPOINT_ELF,
        GUEST_RISC0_L1_BATCH_ELF, GUEST_RISC0_L1_BATCH_ID,
    };
    use strata_state::{
        batch::{BatchInfo, BootstrapState},
        chain_state::ChainState,
    };
    use strata_test_utils::{
        bitcoin::{get_btc_chain, get_tx_filters},
        l2::get_genesis_chainstate,
    };
    use strata_zkvm::{
        AggregationInput, Proof, ProverOptions, VerificationKey, ZkVmHost, ZkVmInputBuilder,
        ZkVmVerifier,
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
            use_cached_keys: true,
        };
        let prover = Risc0Host::init(
            GUEST_RISC0_BTC_BLOCKSPACE_ELF.into(),
            // Default::default(),
            prover_options,
        );

        let btc_chain = get_btc_chain();
        let btc_blockspace_elf_id: Vec<u8> = GUEST_RISC0_BTC_BLOCKSPACE_ID
            .iter()
            .flat_map(|&x| x.to_le_bytes())
            .collect();

        let mut blockspace_outputs = Vec::new();
        let mut prover_input = Risc0ProofInputBuilder::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let filters = get_tx_filters();
            let inner_prover_input = Risc0ProofInputBuilder::new()
                .write_borsh(&filters)
                .unwrap()
                .write_buf(&block_bytes)
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

        let prover = Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF.into(), prover_options);
        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: btc_chain.get_verification_state(40321, &MAINNET.clone().into()),
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
    //         initial_snapshot: get_verification_state_for_block(40_320, &params),
    //         final_state: get_verification_state_for_block(40_321, &params),
    //     }
    // }

    fn l2_snapshot(state: &ChainState) -> ChainStateSnapshot {
        ChainStateSnapshot {
            slot: state.chain_tip_slot(),
            hash: state.compute_state_root(),
            l2_blockid: state.chain_tip_blockid(),
        }
    }

    fn get_l2_batch_output() -> L2BatchProofOutput {
        L2BatchProofOutput {
            deposits: Vec::new(),
            initial_snapshot: l2_snapshot(&get_genesis_chainstate()),
            final_snapshot: l2_snapshot(&get_genesis_chainstate()),
            rollup_params_commitment: Buf32::zero(), // FIXME
        }
    }

    fn get_bootstrap_checkpoint() -> BootstrapState {
        let gen_chainstate = get_genesis_chainstate();
        let btc_chain = get_btc_chain();

        let idx = 1;
        let starting_l1_height = 40321;
        let starting_l2_height = gen_chainstate.chain_tip_slot();

        BootstrapState::new(
            idx,
            starting_l1_height,
            btc_chain
                .get_verification_state(40321, &MAINNET.clone().into())
                .compute_hash()
                .unwrap(),
            starting_l2_height,
            gen_chainstate.compute_state_root(),
            0,
        )
    }

    #[test]
    fn test_checkpoint_proof() {
        let (l1_batch, l1_batch_proof) = get_l1_batch_output_and_proof();
        // let l1_batch = get_l1_batch_output();
        let l2_batch = get_l2_batch_output();

        let bootstrap_checkpoint = get_bootstrap_checkpoint();
        let prover_options = ProverOptions {
            use_mock_prover: false,
            stark_to_snark_conversion: false,
            enable_compression: false,
            use_cached_keys: true,
        };
        let prover = Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF.into(), prover_options);

        let l1_batch_image_id: Vec<u8> = GUEST_RISC0_L1_BATCH_ID
            .iter()
            .flat_map(|&x| x.to_le_bytes())
            .collect();
        let l1_batch_proof_input = AggregationInput::new(
            l1_batch_proof,
            VerificationKey::new(l1_batch_image_id.clone()),
        );

        let prover_input = Risc0ProofInputBuilder::new()
            .write_borsh(&l1_batch)
            .unwrap()
            .write_buf(&borsh::to_vec(&l2_batch).unwrap())
            .unwrap()
            .write_buf(&borsh::to_vec(&bootstrap_checkpoint).unwrap())
            .unwrap()
            .write_proof(l1_batch_proof_input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let output_raw = Risc0Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: BatchInfo = borsh::from_slice(&output_raw).unwrap();
    }
}
