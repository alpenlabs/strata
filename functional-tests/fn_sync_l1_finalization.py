import time

import flexitest

from constants import FAST_BATCH_ROLLUP_PARAMS


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """
    This will test that l2 block indexed at batch_size is finalized after certain l1 height.
    This is because, we create block batches at every batch_size interval.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("fast_batches")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        batch_size = FAST_BATCH_ROLLUP_PARAMS["target_l2_batch_size"]

        # Check for some finalized batches. We need to for blockids at the
        # interval of batch size
        for x in range(1, 5):
            print("Checking finalized for batch", x)
            idx = batch_size * x
            blockid = get_block_at(idx, seqrpc)
            check_finalized(blockid, seqrpc)
            print(f"Batch {x} finalized")


def get_block_at(idx, seqrpc):
    for _ in range(5):  # 5 should be fine for polling block at idx at 0.5s interval
        blocks = seqrpc.alp_getBlockHeadersAtIdx(idx)
        if blocks:
            # NOTE: This assumes that first item is the block we want. This
            # might change when we have multiple sequencers
            return blocks[0]["block_id"]
        time.sleep(0.5)  # 0.5 because this should be good enough time to wait for a block
    raise AssertionError("Did not see block produced within timeout")


def check_finalized(blockid, seqrpc):
    for _ in range(20):  # 20 should be fine for polling finalized blocks at 0.2s interval
        client_stat = seqrpc.alp_clientStatus()
        finalized_id = client_stat["finalized_blkid"]
        if finalized_id == blockid:
            return True
        time.sleep(0.2)
    raise AssertionError("Did not see finalized blockid within timeout")
