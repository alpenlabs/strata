use risc0_zkvm::guest::env;
use strata_primitives::{
    block_credential,
    buf::Buf32,
    params::{Params, RollupParams, SyncParams},
    vk::RollupVerifyingKey,
};
use strata_proofimpl_cl_stf::{verify_and_transition, Chainstate, L2Block};

fn main() {
    let params = get_rollup_params();
    let input: Vec<u8> = env::read();
    let (prev_state, block): (Chainstate, L2Block) = borsh::from_slice(&input).unwrap();

    let new_state = verify_and_transition(prev_state, block, params);
    env::commit(&borsh::to_vec(&new_state).unwrap());
}

// TODO: Should be read from config file and evaluated on compile time
fn get_rollup_params() -> Params {
    Params {
        rollup: RollupParams {
            rollup_name: "strata".to_string(),
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
            rollup_vk: RollupVerifyingKey::Risc0VerifyingKey(Buf32(
                "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
                    .parse()
                    .unwrap(),
            )),
        },
        run: SyncParams {
            l2_blocks_fetch_limit: 1000,
            l1_follow_distance: 3,
            client_checkpoint_interval: 10,
        },
    }
}
