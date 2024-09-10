use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    params::{Params, RollupParams, RunParams},
};
use express_cl_stf::{verify_and_transition, ChainState, L2Block};

fn main() {
    let params = get_rollup_params();
    let input: Vec<u8> = sp1_zkvm::io::read();
    let (prev_state, block): (ChainState, L2Block) = borsh::from_slice(&input).unwrap();

    let new_state = verify_and_transition(prev_state, block, params);
    sp1_zkvm::io::commit(&borsh::to_vec(&new_state).unwrap());
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
            batch_l2_blocks_target: 64,
        },
        run: RunParams {
            l2_blocks_fetch_limit: 1000,
            l1_follow_distance: 3,
            client_checkpoint_interval: 10,
        },
    }
}

const PARAMS: Params = Params {
    rollup: RollupParams {
        rollup_name: String::from("express"),
        block_time: 1000,
        cred_rule: block_credential::CredRule::Unchecked,
        horizon_l1_height: 3,
        genesis_l1_height: 5,
        evm_genesis_block_hash: Buf32::from([
            0x37, 0xad, 0x61, 0xcf, 0xf1, 0x36, 0x74, 0x67, 0xa9, 0x8c, 0xf7, 0xc5, 0x4c, 0x4a,
            0xc9, 0x9e, 0x98, 0x9f, 0x1f, 0xbb, 0x1b, 0xc1, 0xe6, 0x46, 0x23, 0x5e, 0x90, 0xc0,
            0x65, 0xc5, 0x65, 0xba,
        ]),
        evm_genesis_block_state_root: Buf32::from([
            0x35, 0x17, 0x14, 0xaf, 0x72, 0xd7, 0x42, 0x59, 0xf4, 0x5c, 0xd7, 0xea, 0xb0, 0xb0,
            0x45, 0x27, 0xcd, 0x40, 0xe7, 0x48, 0x36, 0xa4, 0x5a, 0xbc, 0xae, 0x50, 0xf9, 0x2d,
            0x91, 0x9d, 0x98, 0x8f,
        ]),
        l1_reorg_safe_depth: 5,
        batch_l2_blocks_target: 64,
    },
    run: RunParams {
        l2_blocks_fetch_limit: 1000,
        l1_follow_distance: 3,
        client_checkpoint_interval: 10,
    },
};
