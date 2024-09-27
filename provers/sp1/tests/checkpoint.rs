#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {

    use alpen_express_state::{
        batch::{BootstrapCheckpoint, CheckpointInfo},
        chain_state::ChainState,
    };
    use alpen_test_utils::{
        bitcoin::{get_btc_chain, get_tx_filters},
        l2::get_genesis_chainstate,
    };
    use bitcoin::params::MAINNET;
    use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use express_proofimpl_checkpoint::{
        ChainStateSnapshot, HashedCheckpointState, L2BatchProofOutput,
    };
    use express_proofimpl_l1_batch::{
        logic::{L1BatchProofInput, L1BatchProofOutput},
        mock::get_verification_state_for_block,
    };
    use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use express_sp1_guest_builder::{
        GUEST_BTC_BLOCKSPACE_ELF, GUEST_CHECKPOINT_ELF, GUEST_L1_BATCH_ELF,
    };
    use express_zkvm::{
        AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
        ZKVMVerifier,
    };

    // TODO: handle this repeat
    fn get_l1_batch_output_and_proof() -> (L1BatchProofOutput, Proof, VerificationKey) {
        let mainnet_blocks: Vec<(u32, String)> = vec![
            (40321, "0100000045720d24eae33ade0d10397a2e02989edef834701b965a9b161e864500000000993239a44a83d5c427fd3d7902789ea1a4d66a37d5848c7477a7cf47c2b071cd7690784b5746651c3af7ca030101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c02db00ffffffff0100f2052a01000000434104c9f513361104db6a84fb6d5b364ba57a27cd19bd051239bf750d8999c6b437220df8fea6b932a248df3cad1fdebb501791e02b7b893a44718d696542ba92a0acac00000000".to_owned()),
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
        let mut prover_input = SP1ProofInputBuilder::new();
        for (_, raw_block) in mainnet_blocks {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let filters = get_tx_filters();
            let inner_prover_input = SP1ProofInputBuilder::new()
                .write_borsh(&filters)
                .unwrap()
                .write_serialized(&block_bytes)
                .unwrap()
                .build()
                .unwrap();

            let (proof, vkey) = prover
                .prove(inner_prover_input)
                .expect("Failed to generate proof");

            let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
                .expect("Failed to extract public outputs");
            let output: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();

            let _ = prover_input.write_proof(AggregationInput::new(proof, vkey));
            blockspace_outputs.push(output);
        }

        let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), prover_options);
        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: get_verification_state_for_block(40321, &MAINNET),
        };

        let prover_input = prover_input.write_borsh(&input).unwrap().build().unwrap();
        let (proof, vk) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let output_raw = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let output: L1BatchProofOutput = borsh::from_slice(&output_raw).unwrap();

        (output, proof, vk)
    }

    // fn get_l1_batch_output() -> L1BatchProofOutput {
    //     let params = PowParams::from(&MAINNET);
    //     L1BatchProofOutput {
    //         deposits: Vec::new(),
    //         forced_inclusions: Vec::new(),
    //         state_update: None,
    //         initial_snapshot: get_verification_state_for_block(40_320, &params),
    //         final_snapshot: get_verification_state_for_block(40_321, &params),
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
        }
    }

    fn get_bootstrap_checkpoint() -> BootstrapCheckpoint {
        let gen_chainstate = get_genesis_chainstate();
        let btc_chain = get_btc_chain();

        let idx = 1;
        let starting_l1_height = 40321;
        let starting_l2_height = gen_chainstate.chain_tip_slot();
        let starting_l2_blkid = gen_chainstate.chain_tip_blockid();

        let info = BootstrapCheckpointInfo::new(
            idx,
            starting_l1_height,
            starting_l2_height,
            starting_l2_blkid,
        );

        let state = BootstrapCheckpointState::new(
            btc_chain
                .get_verification_state(40321, &MAINNET)
                .hash()
                .unwrap(),
            gen_chainstate.compute_state_root(),
            0,
        );

        BootstrapCheckpoint { info, state }
    }

    #[test]
    fn test_checkpoint_proof() {
        let (l1_batch, l1_batch_proof, l1_batch_vk) = get_l1_batch_output_and_proof();
        let l2_batch = get_l2_batch_output();

        let bootstrap_checkpoint = get_bootstrap_checkpoint();

        let prover_options = ProverOptions {
            use_mock_prover: true,
            stark_to_snark_conversion: false,
            enable_compression: false,
        };
        let prover = SP1Host::init(GUEST_CHECKPOINT_ELF.into(), prover_options);

        let l1_batch_proof_input = AggregationInput::new(l1_batch_proof, l1_batch_vk);

        let prover_input = SP1ProofInputBuilder::new()
            .write_borsh(&l1_batch)
            .unwrap()
            .write_serialized(&borsh::to_vec(&l2_batch).unwrap())
            .unwrap()
            .write_serialized(&borsh::to_vec(&bootstrap_checkpoint).unwrap())
            .unwrap()
            .write_proof(l1_batch_proof_input)
            .unwrap()
            .build()
            .unwrap();

        let (proof, _) = prover
            .prove(prover_input)
            .expect("Failed to generate proof");

        let output_raw = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let _: CheckpointInfo = borsh::from_slice(&output_raw).unwrap();
    }
}
