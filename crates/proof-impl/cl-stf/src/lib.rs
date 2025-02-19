//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

pub mod prover;

use borsh::{BorshDeserialize, BorshSerialize};
use strata_chaintsn::transition::process_block;
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_proofimpl_btc_blockspace::logic::BlockscanProofOutput;
use strata_state::{
    block::{ExecSegment, L2Block},
    block_validation::{check_block_credential, validate_block_segments},
    chain_state::Chainstate,
    header::L2Header,
    l1::L1HeaderPayload,
    state_op::StateCache,
    tx::{DaCommitment, DepositInfo, ProtocolOperation},
};
use zkaleido::ZkVmEnv;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ClStfOutput {
    initial_epoch: u64,
    initial_chainstate_root: Buf32,
    final_epoch: u64,
    final_chainstate_root: Buf32,
}

pub fn process_cl_stf(zkvm: &impl ZkVmEnv, el_vkey: &[u32; 8], btc_blockscan_vkey: &[u32; 8]) {
    // 1. Read the rollup params
    let rollup_params: RollupParams = zkvm.read_serde();

    // 2. Read the initial chainstate from which we start the transition and create the state cache
    let initial_chainstate: Chainstate = zkvm.read_borsh();
    let initial_chainstate_root = initial_chainstate.compute_state_root();
    let initial_epoch = initial_chainstate.cur_epoch();
    let mut state_cache = StateCache::new(initial_chainstate);

    // 3. Read L2 blocks
    let l2_blocks: Vec<L2Block> = zkvm.read_borsh();
    assert!(!l2_blocks.is_empty(), "At least one L2 block is required");

    // 4. Read the verified exec segments
    // This is the expected output of EVM EE STF Proof
    // Right now, each L2 block must contain exactly one ExecSegment, but this may change in the
    // future
    let exec_segments: Vec<ExecSegment> = zkvm.read_verified_borsh(el_vkey);
    assert_eq!(
        l2_blocks.len(),
        exec_segments.len(),
        "mismatch len of l2 block and exec segments"
    );

    // 5. Read the verified blockscan proof outputs
    // This is the expected output of L1 Blockscan Proof
    let l1_updates: BlockscanProofOutput = zkvm.read_verified_borsh(btc_blockscan_vkey);

    // Track the current index for Blockscan result
    // This index are necessary because while each ExecSegment in L2BlockBody corresponds
    // directly to an L2 block, an L1Segment may be absent, or there may be multiple per L2 block.
    let mut blockscan_result_idx = 0;

    for (l2_block, exec_segment) in l2_blocks.iter().zip(exec_segments) {
        // 6. Verify that the exec segment is the same that was proven
        assert_eq!(
            l2_block.exec_segment(),
            &exec_segment,
            "mismatch between exec segment at height {}",
            l2_block.header().blockidx()
        );

        // 7. Verify that the L1 payloads are consistent with the one that was proven
        // Since only some information of the L1HeaderPayload is verified by the Blockspace Proof,
        // verify only those parts
        let new_l1_payloads = l2_block.l1_segment().new_payloads();
        for payload in new_l1_payloads {
            let deposits = deposits_in_payload(payload);
            let da_commitments = da_commitments_in_payload(payload);

            // 7a. Verify that the deposits matches
            assert_eq!(
                &l1_updates.blockscan_results[blockscan_result_idx].deposits,
                &deposits,
                "mismatch between deposits at L1 height {}",
                payload.idx()
            );

            // 7b. Verify that the DA commitment matches
            assert_eq!(
                &l1_updates.blockscan_results[blockscan_result_idx].da_commitments,
                &da_commitments,
                "mismatch between DA commitments at L1 height {}",
                payload.idx()
            );

            // 7b. Verify that the L1 Header matches
            assert_eq!(
                &l1_updates.blockscan_results[blockscan_result_idx].raw_header,
                payload.header_buf(),
                "mismatch between header at L1 height {}",
                payload.idx()
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

    // 11. Get the final chainstate and construct the output
    let (final_chain_state, _) = state_cache.finalize();

    let output = ClStfOutput {
        initial_chainstate_root,
        initial_epoch,
        final_chainstate_root: final_chain_state.compute_state_root(),
        final_epoch: final_chain_state.cur_epoch(),
    };

    zkvm.commit_borsh(&output);
}

fn deposits_in_payload(payload: &L1HeaderPayload) -> Vec<DepositInfo> {
    payload
        .deposit_update_txs()
        .iter()
        .flat_map(|tx| {
            tx.tx().protocol_ops().iter().filter_map(|op| {
                if let ProtocolOperation::Deposit(info) = op {
                    Some(info.clone())
                } else {
                    None
                }
            })
        })
        .collect()
}

fn da_commitments_in_payload(payload: &L1HeaderPayload) -> Vec<DaCommitment> {
    payload
        .da_txs()
        .iter()
        .flat_map(|tx| {
            tx.tx().protocol_ops().iter().filter_map(|op| {
                if let ProtocolOperation::DaCommitment(commitment) = op {
                    Some(*commitment)
                } else {
                    None
                }
            })
        })
        .collect()
}
