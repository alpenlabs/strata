from bitcoinlib.keys import Key

BD_USERNAME = "alpen"
BD_PASSWORD = "alpen"
DD_ROOT = "_dd"
# keep in sync with `strata-consensus-logic::genesis::MAX_HORIZON_POLL_INTERVAL`
MAX_HORIZON_POLL_INTERVAL_SECS = 1
SEQ_SLACK_TIME_SECS = 2  # to account for thread sync and startup times
BLOCK_GENERATION_INTERVAL_SECS = 0.5
# BLOCK_GENERATION_INTERVAL_SECS = 10
SEQ_PUBLISH_BATCH_INTERVAL_SECS = 5

# Error codes
ERROR_PROOF_ALREADY_CREATED = -32611
ERROR_CHECKPOINT_DOESNOT_EXIST = -32610

# custom precompiles
PRECOMPILE_BRIDGEOUT_ADDRESS = "0x5400000000000000000000000000000000000001"

# Network times and stuff
DEFAULT_BLOCK_TIME_SEC = 1
DEFAULT_EPOCH_SLOTS = 64
DEFAULT_GENESIS_TRIGGER_HT = 5
DEFAULT_OPERATOR_CNT = 2
DEFAULT_PROOF_TIMEOUT = 5  # Secs

# magic values
# TODO Remove every single one of these
EVM_GENESIS_BLOCK_STATE_HASH = "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
EVM_GENESIS_BLOCK_STATE_ROOT = "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
ROLLUP_VK = "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
SEQ_KEY = b"alpen" + b"_1337" * 5 + b"xx"  # must be 32 bytes
SEQ_PUBKEY = Key(SEQ_KEY.hex()).x_hex

# TODO initialize this with the new genesis tool instead of having it hardcoded
DEFAULT_ROLLUP_PARAMS: dict = {
    "rollup_name": "alpenstrata",
    "block_time": DEFAULT_BLOCK_TIME_SEC * 1000,
    "cred_rule": {
        "schnorr_key": SEQ_PUBKEY,
    },
    "horizon_l1_height": 3,
    "genesis_l1_height": DEFAULT_GENESIS_TRIGGER_HT,
    "evm_genesis_block_hash": EVM_GENESIS_BLOCK_STATE_HASH,
    "evm_genesis_block_state_root": EVM_GENESIS_BLOCK_STATE_ROOT,
    "l1_reorg_safe_depth": 4,
    "target_l2_batch_size": DEFAULT_EPOCH_SLOTS,
    "address_length": 20,
    "deposit_amount": 1_000_000_000,
    "rollup_vk": {
        "risc0_verifying_key": ROLLUP_VK,
    },
    "dispatch_assignment_dur": 64,
    "proof_publish_mode": {
        # use an empty proof in batch after this many seconds of not receiving a proof
        # "timeout": 30,
        "timeout": 60 * 10,
    },
    "max_deposits_in_block": 16,
    "operator_config": {
        "static": [
            {
                "signing_pk": "0x8d86834e6fdb45ba6b7ffd067a27b9e1d67778047581d7ef757ed9e0fa474000",
                "wallet_pk": "0xd678aa2ad5e4f2174025aa7f77cd43ee662ccd94499982d0c322a73d1ffecd79",
            },
            {
                "signing_pk": "0x0abb00b8b17e2798ddebd0ccbb858b6f624a1ff7d93ec15baa8a7be3f136474d",
                "wallet_pk": "0x175cb26def00eaef40650c9d611be6750fb577a10ce1f8f85e2b28c346db89f6",
            },
            {
                "signing_pk": "0x2a4b743dc2393a6ee038350a6ef3a55741e6c78ac6491478d832f4e2a23aa6be",
                "wallet_pk": "0x01a4a0e8d1ae4b10b8dc230d5c330e48940587998b94f76a5e2e73d1444d4afb",
            },
        ]
    },
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
