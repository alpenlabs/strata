import time

import flexitest

from constants import SEQ_SLACK_TIME_SECS


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("fast_batches")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        time.sleep(SEQ_SLACK_TIME_SECS)

        check_send_proof_for_non_existent_batch(seqrpc)

        # Check for checkpoints 1,2,3,4
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
        assert e.code == -32610
    else:
        raise AssertionError("Expected rpc error")


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 1 is already sent
        seqrpc.alp_submitCheckpointProof(1, "abc123")
    except Exception as e:
        assert e.code == -32611
    else:
        raise AssertionError("Expected rpc error")
