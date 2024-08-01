use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    params::{Params, RollupParams, RunParams},
};
use alpen_express_state::client_state::ClientState;

pub fn gen_params() -> Params {
    Params {
        rollup: RollupParams {
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
        },
        run: RunParams {
            l1_follow_distance: 3,
        },
    }
}

pub fn gen_client_state(params: Option<&Params>) -> ClientState {
    let params = match params {
        Some(p) => p,
        None => &gen_params(),
    };
    ClientState::from_genesis_params(
        params.rollup.genesis_l1_height,
        params.rollup.genesis_l1_height,
    )
}
