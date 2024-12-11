use std::time::{SystemTime, UNIX_EPOCH};

use bitcoin::secp256k1::{SecretKey, SECP256K1};
use rand::{rngs::StdRng, SeedableRng};
use strata_consensus_logic::genesis::{make_genesis_block, make_genesis_chainstate};
use strata_primitives::{
    block_credential,
    buf::Buf64,
    operator::OperatorPubkeys,
    params::{OperatorConfig, Params, ProofPublishMode, RollupParams, SyncParams},
    proof::RollupVerifyingKey,
};
use strata_state::{
    block::{L2Block, L2BlockAccessory, L2BlockBody, L2BlockBundle},
    chain_state::Chainstate,
    client_state::ClientState,
    header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
};

use crate::{bitcoin::get_btc_chain, ArbitraryGenerator};

pub fn gen_block(parent: Option<&SignedL2BlockHeader>) -> L2BlockBundle {
    let mut arb = ArbitraryGenerator::new();
    let header: L2BlockHeader = arb.generate();
    let body: L2BlockBody = arb.generate();
    let accessory: L2BlockAccessory = arb.generate();

    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let block_idx = parent.map(|h| h.blockidx() + 1).unwrap_or(0);
    let prev_block = parent.map(|h| h.get_blockid()).unwrap_or_default();
    let timestamp = parent
        .map(|h| h.timestamp() + 100)
        .unwrap_or(current_timestamp);

    let header = L2BlockHeader::new(
        block_idx,
        timestamp,
        prev_block,
        &body,
        *header.state_root(),
    );
    let empty_sig = Buf64::zero();
    let signed_header = SignedL2BlockHeader::new(header, empty_sig);
    let block = L2Block::new(signed_header, body);
    L2BlockBundle::new(block, accessory)
}

pub fn gen_l2_chain(parent: Option<SignedL2BlockHeader>, blocks_num: usize) -> Vec<L2BlockBundle> {
    let mut blocks = Vec::new();
    let mut parent = match parent {
        Some(p) => p,
        None => {
            let p = gen_block(None);
            blocks.push(p.clone());
            p.header().clone()
        }
    };

    for _ in 0..blocks_num {
        let block = gen_block(Some(&parent));
        blocks.push(block.clone());
        parent = block.header().clone()
    }

    blocks
}

pub fn gen_params_with_seed(seed: u64) -> Params {
    let opkeys = make_dummy_operator_pubkeys_with_seed(seed);
    Params {
        rollup: RollupParams {
            rollup_name: "strata".to_string(),
            block_time: 1000,
            cred_rule: block_credential::CredRule::Unchecked,
            horizon_l1_height: 40318,
            genesis_l1_height: 40320, // we have mainnet blocks from this height test-utils
            operator_config: OperatorConfig::Static(vec![opkeys]),
            evm_genesis_block_hash:
                "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                    .parse()
                    .unwrap(),
            evm_genesis_block_state_root:
                "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                    .parse()
                    .unwrap(),
            l1_reorg_safe_depth: 3,
            target_l2_batch_size: 64,
            address_length: 20,
            deposit_amount: 1_000_000_000,
            rollup_vk: RollupVerifyingKey::SP1VerifyingKey(
                "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
                    .parse()
                    .unwrap(),
            ),
            dispatch_assignment_dur: 64,
            proof_publish_mode: ProofPublishMode::Strict,
            max_deposits_in_block: 16,
            network: bitcoin::Network::Regtest,
        },
        run: SyncParams {
            l2_blocks_fetch_limit: 1000,
            l1_follow_distance: 3,
            client_checkpoint_interval: 10,
        },
    }
}

pub fn gen_params() -> Params {
    // TODO: create a random seed if we really need random op_pubkeys every time this is called
    gen_params_with_seed(0)
}

pub fn gen_client_state(params: Option<&Params>) -> ClientState {
    let params = match params {
        Some(p) => p,
        None => &gen_params(),
    };
    ClientState::from_genesis_params(
        params.rollup.horizon_l1_height,
        params.rollup.genesis_l1_height,
    )
}

pub fn make_dummy_operator_pubkeys_with_seed(seed: u64) -> OperatorPubkeys {
    let mut rng = StdRng::seed_from_u64(seed);
    let sk = SecretKey::new(&mut rng);
    let x_only_public_key = sk.x_only_public_key(SECP256K1);
    let (pk, _) = x_only_public_key;
    OperatorPubkeys::new(pk.into(), pk.into())
}

pub fn get_genesis_chainstate() -> Chainstate {
    let params = gen_params();
    // Build the genesis block and genesis consensus states.
    let gblock = make_genesis_block(&params);
    let pregenesis_mfs =
        vec![get_btc_chain().get_block_manifest(params.rollup().horizon_l1_height as u32)];
    make_genesis_chainstate(&gblock, pregenesis_mfs, &params)
}
