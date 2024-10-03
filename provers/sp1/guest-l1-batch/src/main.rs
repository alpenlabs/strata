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
    assert!(num_inputs > 0);

    let initial_snapshot = state.compute_snapshot();
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;
    let mut filters_commitment = None;

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
        prev_checkpoint = prev_checkpoint.or(blkpo.prev_checkpoint);

        // Ensure that the filters commitment used are same for all blocks
        if let Some(filters_comm) = filters_commitment {
            assert_eq!(blkpo.filters_commitment, filters_comm);
        } else {
            filters_commitment = Some(blkpo.filters_commitment);
        }
    }
    let final_snapshot = state.compute_snapshot();

    let output = L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_snapshot,
        final_snapshot,
        filters_commitment: filters_commitment.unwrap(),
    };

    sp1_zkvm::io::commit(&borsh::to_vec(&output).unwrap());
}
