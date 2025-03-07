//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

pub mod program;

use program::ClStfOutput;
use strata_chaintsn::transition::process_block;
use strata_primitives::{
    buf::Buf32, hash::compute_borsh_hash, l1::ProtocolOperation, params::RollupParams,
};
use strata_proofimpl_btc_blockspace::logic::BlockscanProofOutput;
use strata_state::{
    block::{ExecSegment, L2Block},
    block_validation::{check_block_credential, validate_block_segments},
    chain_state::Chainstate,
    header::L2Header,
    state_op::StateCache,
};
use zkaleido::ZkVmEnv;

pub fn process_cl_stf(zkvm: &impl ZkVmEnv, el_vkey: &[u32; 8], btc_blockscan_vkey: &[u32; 8]) {
    // 1. Read the rollup params
    let rollup_params: RollupParams = zkvm.read_serde();

    // 2. Read the initial chainstate from which we start the transition and create the state cache
    let initial_chainstate: Chainstate = zkvm.read_borsh();
    let initial_chainstate_root = initial_chainstate.compute_state_root();
    let cur_epoch = initial_chainstate.cur_epoch();
    let mut state_cache = StateCache::new(initial_chainstate);

    // 3. Read L2 blocks
    let l2_blocks: Vec<L2Block> = zkvm.read_borsh();
    assert!(!l2_blocks.is_empty(), "At least one L2 block is required");

    // 4. Read the verified blockscan proof outputs if any
    let is_l1_segment_present: bool = zkvm.read_serde();
    let (l1_updates, tx_filters) = if is_l1_segment_present {
        let btc_blockspace_proof_output: BlockscanProofOutput =
            zkvm.read_verified_borsh(btc_blockscan_vkey);
        (
            btc_blockspace_proof_output.blockscan_results,
            Some(btc_blockspace_proof_output.tx_filters),
        )
    } else {
        (vec![], None)
    };

    // 5. Read the verified exec segments
    // This is the expected output of EVM EE STF Proof
    // Right now, each L2 block must contain exactly one ExecSegment, but this may change in the
    // future
    let exec_segments: Vec<ExecSegment> = zkvm.read_verified_borsh(el_vkey);
    assert_eq!(
        l2_blocks.len(),
        exec_segments.len(),
        "mismatch len of l2 block and exec segments"
    );

    // Track the current index for Blockscan result
    // This index are necessary because while each ExecSegment in L2BlockBody corresponds
    // directly to an L2 block, an L1Segment may be absent, or there may be multiple per L2 block.
    let mut blockscan_result_idx = 0;

    for (l2_block, exec_segment) in l2_blocks.iter().zip(exec_segments) {
        // 6. Verify that the exec segment is the same that was proven
        assert_eq!(
            l2_block.exec_segment(),
            &exec_segment,
            "mismatch between exec segment at height {:?}",
            l2_block.header().blockidx()
        );

        // 7. Verify that the L1 manifests are consistent with the one that was proven
        // Since only some information of the L1BlockManifest is verified by the Blockspace Proof,
        // verify only those parts
        let new_l1_manifests = l2_block.l1_segment().new_manifests();
        assert_eq!(new_l1_manifests.len(), l1_updates.len());

        for manifest in new_l1_manifests {
            assert_eq!(
                &l1_updates[blockscan_result_idx].raw_header,
                manifest.header(),
                "mismatch headers at idx: {:?}",
                blockscan_result_idx
            );

            // OPTIMIZE: if there's a way to compare things without additional cloned
            let protocol_ops: Vec<ProtocolOperation> = manifest
                .txs()
                .iter()
                .flat_map(|tx| tx.protocol_ops().iter().cloned())
                .collect();

            // 7b. Verify that the protocol ops matches
            assert_eq!(
                &l1_updates[blockscan_result_idx].protocol_ops,
                &protocol_ops,
                "mismatch between protocol ops for {}",
                manifest.blkid()
            );

            // Increase the blockscan result idx
            blockscan_result_idx += 1;
        }

        // 8. Now that the L2 Block body is verified, check that the L2 Block header is consistent
        //    with the body
        assert!(validate_block_segments(l2_block), "block validation failed");

        // 9. Verify that the block credential is valid
        assert!(
            check_block_credential(l2_block.header(), &rollup_params),
            "Block credential verification failed"
        );

        // 10. Apply the state transition
        process_block(
            &mut state_cache,
            l2_block.header(),
            l2_block.body(),
            &rollup_params,
        )
        .expect("failed to process L2 Block");
    }

    // 11. Get the checkpoint that was posted to Bitcoin (if any) and check if we have used the
    //     right TxFilters and udpate it
    // FIXME: The first epoch will not have any SignedCheckpoint on Bitcoin
    // TODO: this makes sense to be somewhere in the chainstate
    let (initial_tx_filter_config_hash, final_tx_filter_config_hash) = if is_l1_segment_present {
        let last_l1_block = l1_updates
            .last()
            .expect("there should be at least one L1 Segment");

        let cp = last_l1_block
            .protocol_ops
            .iter()
            .find_map(|op| match op {
                ProtocolOperation::Checkpoint(cp) => Some(cp),
                _ => None,
            })
            .expect("Must include checkpoint for valid epoch");

        let posted_chainstate: Chainstate =
            borsh::from_slice(cp.checkpoint().sidecar().chainstate())
                .expect("valid chainstate needs to be posted on checkpoint");

        let mut tx_filters = tx_filters.expect("must have tx filters");
        let initial_tx_filters_hash = compute_borsh_hash(&tx_filters);

        // Verify we have used the right TxFilters
        assert_eq!(
            initial_tx_filters_hash,
            cp.checkpoint().batch_transition().tx_filters_transition.1,
            "must use right tx filters"
        );

        tx_filters.update_from_chainstate(&posted_chainstate);
        let final_tx_filters_hash = compute_borsh_hash(&tx_filters);

        (initial_tx_filters_hash, final_tx_filters_hash)
    } else {
        (Buf32::zero(), Buf32::zero())
    };

    // 12. Get the final chainstate and construct the output
    let (final_chain_state, _) = state_cache.finalize();

    let output = ClStfOutput {
        initial_chainstate_root,
        final_chainstate_root: final_chain_state.compute_state_root(),
        initial_tx_filter_config_hash,
        final_tx_filter_config_hash,
    };

    zkvm.commit_borsh(&output);
}
