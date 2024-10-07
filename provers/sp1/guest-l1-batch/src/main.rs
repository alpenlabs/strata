use bitcoin::block::Header;
use sha2::{Digest, Sha256};
use strata_primitives::{hash::compute_borsh_hash, params::RollupParams};
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_state::l1::{get_btc_params, HeaderVerificationState};
use strata_tx_parser::filter::derive_tx_filter_rules;

mod vks;

fn main() {
    let rollup_params: RollupParams = sp1_zkvm::io::read();
    let rollup_params_commitment = rollup_params.compute_hash();

    let state_raw = sp1_zkvm::io::read_vec();
    let mut state: HeaderVerificationState = borsh::from_slice(&state_raw).unwrap();

    let num_inputs: u32 = sp1_zkvm::io::read();
    assert!(num_inputs > 0);

    let tx_filters = derive_tx_filter_rules(&rollup_params).unwrap();
    let tx_filters_commitment = compute_borsh_hash(&tx_filters);

    let initial_snapshot = state.compute_snapshot();
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    let vk = vks::GUEST_BTC_BLOCKSPACE_ELF_ID;
    for _ in 0..num_inputs {
        let blkpo_raw = sp1_zkvm::io::read_vec();

        let blkpo_raw_digest = Sha256::digest(&blkpo_raw);
        sp1_zkvm::lib::verify::verify_sp1_proof(vk, &blkpo_raw_digest.into());

        let blkpo: BlockspaceProofOutput = borsh::from_slice(&blkpo_raw).unwrap();
        let header: Header = bitcoin::consensus::deserialize(&blkpo.header_raw).unwrap();

        state.check_and_update_continuity(&header, &get_btc_params());
        deposits.extend(blkpo.deposits);
        prev_checkpoint = prev_checkpoint.or(blkpo.prev_checkpoint);

        // Ensure that the rollup parameters used are same for all blocks
        assert_eq!(blkpo.tx_filters_commitment, tx_filters_commitment);
        assert_eq!(blkpo.cred_rule, rollup_params.cred_rule);
    }
    let final_snapshot = state.compute_snapshot();

    let output = L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_snapshot,
        final_snapshot,
        rollup_params_commitment,
    };

    sp1_zkvm::io::commit_slice(&borsh::to_vec(&output).unwrap());
}
