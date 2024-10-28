use risc0_zkvm::guest::env;
use sha2::{Digest, Sha256};
use strata_primitives::params::RollupParams;
use strata_proofimpl_checkpoint::{ChainStateSnapshot, L2BatchProofOutput};
use strata_proofimpl_cl_stf::{verify_and_transition, ChainState, L2Block};
use strata_proofimpl_evm_ee_stf::ELProofPublicParams;

mod vks;

fn main() {
    let rollup_params: RollupParams = env::read();

    let input_raw = env::read_slice();
    let (prev_state, block): (ChainState, L2Block) = borsh::from_slice(&input_raw).unwrap();

    // Verify the EL proof
    let el_vkey = vks::GUEST_EVM_EE_STF_ELF_ID;
    let el_pp_raw = env::read_slice();
    let el_pp_raw_digest = Sha256::digest(&el_pp_raw);
    sp1_zkvm::lib::verify::verify_sp1_proof(el_vkey, &el_pp_raw_digest.into());

    // Parse proof public params
    let el_pp_deserialized: ELProofPublicParams = bincode::deserialize(&el_pp_raw).unwrap();

    let (new_state, deposits) = verify_and_transition(
        prev_state.clone(),
        block,
        el_pp_deserialized,
        &rollup_params,
    );

    let initial_snapshot = ChainStateSnapshot {
        hash: prev_state.compute_state_root(),
        slot: prev_state.chain_tip_slot(),
        l2_blockid: prev_state.chain_tip_blockid(),
    };

    let final_snapshot = ChainStateSnapshot {
        hash: new_state.compute_state_root(),
        slot: new_state.chain_tip_slot(),
        l2_blockid: new_state.chain_tip_blockid(),
    };

    let cl_stf_public_params = L2BatchProofOutput {
        deposits,
        final_snapshot,
        initial_snapshot,
        rollup_params_commitment: rollup_params.compute_hash(),
    };

    env::commit(&borsh::to_vec(&cl_stf_public_params).unwrap());
}
