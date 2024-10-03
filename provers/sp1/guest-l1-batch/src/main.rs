use alpen_express_state::l1::{get_btc_params, HeaderVerificationState};
use bitcoin::block::Header;
use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use express_proofimpl_l1_batch::L1BatchProofOutput;
use sha2::{Digest, Sha256};

mod vks;

fn main() {
    let state_raw = sp1_zkvm::io::read_vec();
    let mut state: HeaderVerificationState = borsh::from_slice(&state_raw).unwrap();

    let num_inputs: u32 = sp1_zkvm::io::read();

    let initial_snapshot = state.compute_snapshot();
    let mut deposits = Vec::new();
    let mut state_update = None;

    let vk = vks::GUEST_BTC_BLOCKSPACE_ELF_ID;
    for _ in 0..num_inputs {
        let blkpo_raw: Vec<u8> = sp1_zkvm::io::read();

        let public_values_digest = Sha256::digest(&blkpo_raw);
        sp1_zkvm::lib::verify::verify_sp1_proof(vk, &public_values_digest.into());

        let blkpo_raw_serialized: Vec<u8> = bincode::deserialize(&blkpo_raw).unwrap();
        let blkpo: BlockspaceProofOutput = borsh::from_slice(&blkpo_raw_serialized).unwrap();
        let header: Header = bitcoin::consensus::deserialize(&blkpo.header_raw).unwrap();

        state.check_and_update_continuity(&header, &get_btc_params());
        deposits.extend(blkpo.deposits);
        state_update = state_update.or(blkpo.state_update);
    }
    let final_snapshot = state.compute_snapshot();

    let output = L1BatchProofOutput {
        deposits,
        state_update,
        initial_snapshot,
        final_snapshot,
    };

    sp1_zkvm::io::commit(&borsh::to_vec(&output).unwrap());
}
