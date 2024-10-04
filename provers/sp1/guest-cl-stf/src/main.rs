use alpen_express_primitives::params::RollupParams;
use express_proofimpl_checkpoint::{ChainStateSnapshot, L2BatchProofOutput};
use express_proofimpl_cl_stf::{verify_and_transition, ChainState, L2Block};
use express_proofimpl_evm_ee_stf::ELProofPublicParams;
use sha2::{Digest, Sha256};

mod vks;

fn main() {
    let rollup_params: RollupParams = sp1_zkvm::io::read();
    let el_vkey = vks::GUEST_EVM_EE_STF_ELF_ID;

    let el_pp = sp1_zkvm::io::read::<Vec<u8>>();
    let input: Vec<u8> = sp1_zkvm::io::read();
    let (prev_state, block): (ChainState, L2Block) = borsh::from_slice(&input).unwrap();

    // Verify the EL proof
    let public_values_digest = Sha256::digest(&el_pp);
    sp1_zkvm::lib::verify::verify_sp1_proof(el_vkey, &public_values_digest.into());
    let el_pp_deserialized: ELProofPublicParams = bincode::deserialize(&el_pp).unwrap();

    let new_state = verify_and_transition(
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
        // TODO: Accumulate the deposits
        deposits: Vec::new(),
        final_snapshot,
        initial_snapshot,
        rollup_params_commitment: rollup_params.compute_hash(),
    };

    sp1_zkvm::io::commit(&borsh::to_vec(&cl_stf_public_params).unwrap());
}
