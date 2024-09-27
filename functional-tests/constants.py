BD_USERNAME = "alpen"
BD_PASSWORD = "alpen"
DD_ROOT = "_dd"
# keep in sync with `express-consensus-logic::genesis::MAX_HORIZON_POLL_INTERVAL`
MAX_HORIZON_POLL_INTERVAL_SECS = 1
SEQ_SLACK_TIME_SECS = 2  # to account for thread sync and startup times
BLOCK_GENERATION_INTERVAL_SECS = 0.5
SEQ_PUBLISH_BATCH_INTERVAL_SECS = 5

common_params = {
    "rollup_name": "expresssss",
    "block_time": 1000,
    "cred_rule": "Unchecked",
    "operator_config": {
        "Static": [
            {
                "signing_pk": "01" * 32,
                "wallet_pk": "02" * 32,
            },
            {
                "signing_pk": "03" * 32,
                "wallet_pk": "04" * 32,
            },
        ]
    },
    "evm_genesis_block_hash": "37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba",
    "evm_genesis_block_state_root": (
        "351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
    ),
    "l1_reorg_safe_depth": 4,
    "rollup_vk": {
        "risc0_verifying_key": "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
    },
    "address_length": 20,
    "verify_proofs": False,
    "dispatch_assignment_dur": 64,
    "proof_publish_mode": "Strict",
    "deposit_amount": 10**7,
    "max_deposits_in_block": 16,
}

FAST_BATCH_ROLLUP_PARAMS = {
    **common_params,
    "horizon_l1_height": 3,
    "target_l2_batch_size": 5,
    "genesis_l1_height": 5,
}

ROLLUP_BATCH_WITH_FUNDS = {
    **common_params,
    "horizon_l1_height": 4,
    "target_l2_batch_size": 100,
    "genesis_l1_height": 102,
}

# Error codes
ERROR_PROOF_ALREADY_CREATED = -32611
ERROR_CHECKPOINT_DOESNOT_EXIST = -32610

# custom precompiles
PRECOMPILE_BRIDGEOUT_ADDRESS = "0x5400000000000000000000000000000000000001"
