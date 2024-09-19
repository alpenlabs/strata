import time

import flexitest

from constants import (
    ERROR_CHECKPOINT_DOESNOT_EXIST,
    ERROR_PROOF_ALREADY_CREATED,
    FAST_BATCH_ROLLUP_PARAMS,
)
from entry import BasicEnvConfig


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_send_proof_for_non_existent_batch(seqrpc)

        # Check for first 4 checkpoints
        for n in range(1, 5):
            check_for_nth_checkpoint_finalization(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_for_nth_checkpoint_finalization(idx, seqrpc):
    syncstat = seqrpc.alp_syncStatus()
    checkpoint_info = seqrpc.alp_getCheckpointInfo(idx)
    print(f"checkpoint info for {idx}", checkpoint_info)

    assert (
        syncstat["finalized_block_id"] != checkpoint_info["l2_blockid"]
    ), "Checkpoint block should not yet finalize"

    checkpoint_info_1 = seqrpc.alp_getCheckpointInfo(idx + 1)

    assert checkpoint_info["idx"] == idx
    assert checkpoint_info_1 is None, f"There should be no checkpoint info for {idx + 1} index"

    to_finalize_blkid = checkpoint_info["l2_blockid"]

    # Post checkpoint proof
    proof_hex = "abcdef"
    seqrpc.alp_submitCheckpointProof(idx, proof_hex)

    # Wait till checkpoint finalizes, since our finalization depth is 4 and the block
    # generation time is 0.5s wait slightly more than 2 secs
    # Ideally this should be tested with controlled bitcoin block production
    time.sleep(4)

    syncstat = seqrpc.alp_syncStatus()
    print("Sync Stat", syncstat)
    assert to_finalize_blkid == syncstat["finalized_block_id"], "Block not finalized"


def check_send_proof_for_non_existent_batch(seqrpc):
    try:
        seqrpc.alp_submitCheckpointProof(100, "abc123")
    except Exception as e:
        assert e.code == ERROR_CHECKPOINT_DOESNOT_EXIST
    else:
        raise AssertionError("Expected rpc error")


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 1 is already sent
        seqrpc.alp_submitCheckpointProof(1, "abc123")
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
