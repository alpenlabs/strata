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

        l1status = seqrpc.alp_l1status()
        last_published = l1status["last_published_txid"]
        while not last_published:
            l1status = seqrpc.alp_l1status()
            last_published = l1status["last_published_txid"]
        print("at height", l1status["cur_height"], "LAST PUBLISHED", last_published)

        # check for some finalized batches
        for x in range(1, 5):
            print("Checking finalized for batch", x)
            idx = batch_size * x
            blockid = get_block_at(idx, seqrpc)
            check_finalized(blockid, seqrpc)
            print(f"Batch {x} finalized")


def get_block_at(idx, seqrpc):
        counter = 0
        # We expect to get block at idx within 5 loops
        while counter < 5:
            blocks = seqrpc.alp_getBlocksAtIdx(idx)
            if blocks:
                return blocks[0]["block_id"]
            time.sleep(0.5)
        assert False, "could not see block produced within timeout"


def check_finalized(blockid, seqrpc):
    counter = 0
    # We expect to see block finalized within 20 loops.
    while counter < 20:
        client_stat = seqrpc.alp_clientStatus()
        finalized_id = client_stat["finalized_blkid"]
        if finalized_id == blockid:
            return True
        time.sleep(0.4)
        counter += 1
    assert False, "Did not see finalized blockid"
