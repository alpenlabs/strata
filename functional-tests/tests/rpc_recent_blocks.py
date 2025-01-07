import time

import flexitest

from envs import testenv

NUM_BLOCKS_TO_RECEIVE = 10
CHECK_BLOCK_NUMBER = 6
TRY_LIMIT = 20


@flexitest.register
class RecentBlocksTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        counter = 0
        while counter < TRY_LIMIT:
            blk = seqrpc.strata_getHeadersAtIdx(NUM_BLOCKS_TO_RECEIVE)
            if blk is not None:
                break

            counter += 1
            time.sleep(1)

        assert counter < TRY_LIMIT, "seem to be not making progress"

        recent_blks = seqrpc.strata_getRecentBlockHeaders(NUM_BLOCKS_TO_RECEIVE)
        assert len(recent_blks) == NUM_BLOCKS_TO_RECEIVE, f"got {len(recent_blks)} blocks back, asked for {NUM_BLOCKS_TO_RECEIVE}"

        # check if they are in order by verifying if N-1 block is parent of N block
        for idx in reversed(range(0, NUM_BLOCKS_TO_RECEIVE)):
            if idx != NUM_BLOCKS_TO_RECEIVE - 1:
                assert recent_blks[idx]["prev_block"] == recent_blks[idx + 1]["block_id"]

        l2_blk = seqrpc.strata_getHeadersAtIdx(recent_blks[CHECK_BLOCK_NUMBER]["block_idx"])

        assert recent_blks[CHECK_BLOCK_NUMBER]["block_idx"] == l2_blk[0]["block_idx"]

        second_blk_from_id = seqrpc.strata_getHeaderById(l2_blk[0]["block_id"])

        # check if we got the correct block when looked using hash
        assert second_blk_from_id["block_id"] == l2_blk[0]["block_id"]
