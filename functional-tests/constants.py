from bitcoinlib.keys import Key

BD_USERNAME = "alpen"
BD_PASSWORD = "alpen"
DD_ROOT = "_dd"
# keep in sync with `strata-consensus-logic::genesis::MAX_HORIZON_POLL_INTERVAL`
MAX_HORIZON_POLL_INTERVAL_SECS = 1
SEQ_SLACK_TIME_SECS = 2  # to account for thread sync and startup times
BLOCK_GENERATION_INTERVAL_SECS = 0.5
SEQ_PUBLISH_BATCH_INTERVAL_SECS = 5

# Error codes
ERROR_PROOF_ALREADY_CREATED = -32611
ERROR_CHECKPOINT_DOESNOT_EXIST = -32610

# custom precompiles
PRECOMPILE_BRIDGEOUT_ADDRESS = "0x5400000000000000000000000000000000000001"

# magic values
EVM_GENESIS_BLOCK_STATE_HASH = "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
EVM_GENESIS_BLOCK_STATE_ROOT = "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
ROLLUP_VK = "0x000790761b53d26cc7920b6562ac0315ea81a33f15ee6cbe5d5fb504f48c7481"
SEQ_KEY = b"alpen" + b"_1337" * 5 + b"xx"  # must be 32 bytes
SEQ_PUBKEY = Key(SEQ_KEY.hex()).x_hex

# TODO initialize this with the new genesis tool instead of having it hardcoded
DEFAULT_ROLLUP_PARAMS: dict = {
    "rollup_name": "strata",
    "block_time": 1000,
    "cred_rule": {
        "schnorr_key": SEQ_PUBKEY,
    },
    "horizon_l1_height": 3,
    "genesis_l1_height": 5,
    "evm_genesis_block_hash": EVM_GENESIS_BLOCK_STATE_HASH,
    "evm_genesis_block_state_root": EVM_GENESIS_BLOCK_STATE_ROOT,
    "l1_reorg_safe_depth": 4,
    "target_l2_batch_size": 64,
    "address_length": 20,
    "deposit_amount": 1_000_000_000,
    "rollup_vk": {
        "s_p1_verifying_key": ROLLUP_VK,
    },
    "dispatch_assignment_dur": 64,
    "proof_publish_mode": {
        # use an empty proof in batch after this many seconds of not receiving a proof
        # "timeout": 30,
        "timeout": 60 * 10,
    },
    "max_deposits_in_block": 16,
    "operator_config": {"static": [{"signing_pk": "01" * 32, "wallet_pk": "02" * 32}]},
    "network": "regtest",
}

FAST_BATCH_ROLLUP_PARAMS = {
    **DEFAULT_ROLLUP_PARAMS,
    "target_l2_batch_size": 5,
    "genesis_l1_height": 5,
}

# static operator config with pregenerated 100 blocks for deposit transaction
ROLLUP_PARAMS_FOR_DEPOSIT_TX = {
    **DEFAULT_ROLLUP_PARAMS,
    "horizon_l1_height": 4,
    "target_l2_batch_size": 100,
    "genesis_l1_height": 102,
    "operator_config": {
        "static": [
            {
                "signing_pk": "01" * 32,
                "wallet_pk": "02b4634c515a62e47b3f3eb62b8a6f6320fdb2baed5f2e6657f472b0f2a33221",
            }
        ]
    },
}

PROVER_ROLLUP_PARAMS = {
    **DEFAULT_ROLLUP_PARAMS,
    "proof_publish_mode": "strict",
    "genesis_l1_height": 500,
    "cred_rule": "unchecked",
    "operator_config": {
        "static": [
            {
                "signing_pk": "1ea7f6c7a924a1e722c09301a826cfd95b21b23a294cec5da3c0b5908e62e0c7",
                "wallet_pk": "1ea7f6c7a924a1e722c09301a826cfd95b21b23a294cec5da3c0b5908e62e0c7",
            }
        ]
    },
}
