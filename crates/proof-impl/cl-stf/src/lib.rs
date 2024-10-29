//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

use strata_primitives::{
    buf::Buf32,
    evm_exec::create_evm_extra_payload,
    l1::{BitcoinAmount, XOnlyPk},
    params::RollupParams,
};
use strata_proofimpl_evm_ee_stf::ELProofPublicParams;
use strata_state::{
    block::ExecSegment,
    block_validation::validate_block_segments,
    bridge_ops,
    exec_update::{ELDepositData, ExecUpdate, Op, UpdateInput, UpdateOutput},
};
pub use strata_state::{block::L2Block, chain_state::ChainState, state_op::StateCache};

/// Verifies an L2 block and applies the chain state transition if the block is valid.
pub fn verify_and_transition(
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    el_proof_pp: ELProofPublicParams,
    rollup_params: &RollupParams,
) -> (ChainState, Vec<ELDepositData>) {
    let deposit_datas = verify_l2_block(&new_l2_block, &el_proof_pp);
    let new_state = apply_state_transition(prev_chstate, &new_l2_block, rollup_params);

    (new_state, deposit_datas)
}

/// Verifies the L2 block.
fn verify_l2_block(block: &L2Block, el_proof_pp: &ELProofPublicParams) -> Vec<ELDepositData> {
    // Assert that the block body and header are consistent
    assert!(
        validate_block_segments(block),
        "Block credential verification failed"
    );

    // Verify proof public params matches the exec segment
    let (proof_exec_segment, el_deposit_data) = reconstruct_exec_segment(el_proof_pp);
    let block_exec_segment = block.body().exec_segment().clone();
    assert_eq!(proof_exec_segment, block_exec_segment);

    el_deposit_data
}

/// Generates an execution segment from the given ELProof public parameters.
pub fn reconstruct_exec_segment(
    el_proof_pp: &ELProofPublicParams,
) -> (ExecSegment, Vec<ELDepositData>) {
    let withdrawals = el_proof_pp
        .withdrawal_intents
        .iter()
        .map(|intent| {
            bridge_ops::WithdrawalIntent::new(
                BitcoinAmount::from_sat(intent.amt),
                XOnlyPk::new(Buf32(intent.dest_pk)),
            )
        })
        .collect::<Vec<_>>();

    let el_deposit_data: Vec<ELDepositData> = el_proof_pp
        .deposit_requests
        .iter()
        .map(|deposit_request| {
            ELDepositData::new(
                deposit_request.index,
                gwei_to_sats(deposit_request.amount),
                deposit_request.address.as_slice().to_vec(),
            )
        })
        .collect();

    let applied_ops = el_deposit_data
        .iter()
        .map(|deposit_data| Op::Deposit(deposit_data.clone()))
        .collect();

    let update_input = UpdateInput::new(
        el_proof_pp.block_idx,
        applied_ops,
        Buf32(el_proof_pp.txn_root),
        create_evm_extra_payload(Buf32(el_proof_pp.new_blockhash)),
    );

    let update_output = UpdateOutput::new_from_state(Buf32(el_proof_pp.new_state_root))
        .with_withdrawals(withdrawals);
    let exec_update = ExecUpdate::new(update_input, update_output);

    (ExecSegment::new(exec_update), el_deposit_data)
}

/// Applies a state transition for a given L2 block.
fn apply_state_transition(
    prev_chstate: ChainState,
    new_l2_block: &L2Block,
    chain_params: &RollupParams,
) -> ChainState {
    let mut state_cache = StateCache::new(prev_chstate);

    strata_chaintsn::transition::process_block(
        &mut state_cache,
        new_l2_block.header(),
        new_l2_block.body(),
        chain_params,
    )
    .expect("Failed to process the L2 block");

    state_cache.state().to_owned()
}

const fn gwei_to_sats(gwei: u64) -> u64 {
    // 1 BTC = 10^8 sats = 10^9 gwei
    gwei / 10
}
