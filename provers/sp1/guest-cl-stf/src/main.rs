use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    params::{Params, RollupParams, SyncParams},
    vk::RollupVerifyingKey,
};
use express_proofimpl_cl_stf::{verify_and_transition, CLProofInput, CLProofPublicParams};
use express_proofimpl_evm_ee_stf::ELProofPublicParams;
use sha2::{Digest, Sha256};

// mod vks;

fn main() {
    println!("Wokring on this abishek");
    // let params = get_rollup_params();

    // Read the input from the host
    // let el_vkey = sp1_zkvm::io::read::<[u32; 8]>();
    // let el_pp = sp1_zkvm::io::read::<Vec<u8>>();
    // let cl_input: Vec<u8> = sp1_zkvm::io::read();
    // let (prev_state, block): CLProofInput = borsh::from_slice(&cl_input).unwrap();

    // Verify the EL proof
    // let public_values_digest = Sha256::digest(&el_pp);
    // sp1_zkvm::lib::verify::verify_sp1_proof(&el_vkey, &public_values_digest.into());
    // let el_pp_deserialized: ELProofPublicParams = bincode::deserialize(&el_pp).unwrap();
    // let prev_state_root = prev_state.compute_state_root();

    // // Verify the CL block proof
    // let new_state = verify_and_transition(prev_state, block, el_pp_deserialized, params);
    // let new_state_root = new_state.compute_state_root();

    // let public_params: CLProofPublicParams = (*prev_state_root.as_ref(),
    // *new_state_root.as_ref()); sp1_zkvm::io::commit(&public_params);
}

// TODO: Should be read from config file and evaluated on compile time
fn get_rollup_params() -> Params {
    Params {
        rollup: RollupParams {
            rollup_name: "express".to_string(),
            block_time: 1000,
            cred_rule: block_credential::CredRule::Unchecked,
            horizon_l1_height: 3,
            genesis_l1_height: 5,
            evm_genesis_block_hash: Buf32(
                "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                    .parse()
                    .unwrap(),
            ),
            evm_genesis_block_state_root: Buf32(
                "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                    .parse()
                    .unwrap(),
            ),
            l1_reorg_safe_depth: 5,
            target_l2_batch_size: 64,
            address_length: 20,
            deposit_amount: 1_000_000_000,
            verify_proofs: false,
            rollup_vk: RollupVerifyingKey::SP1VerifyingKey(Buf32(
                "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
                    .parse()
                    .unwrap(),
            )),
            verify_proofs: true,
        },
        run: SyncParams {
            l2_blocks_fetch_limit: 1000,
            l1_follow_distance: 3,
            client_checkpoint_interval: 10,
        },
    }
}
