import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest

NUM_BLOCKS_TO_RECEIVE = 10
BLOCK_NUMBER = 4


@flexitest.register
class RecentBlocksTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        counter = 0
        while True:
            blk = seqrpc.alp_getBlocksAtIdx(NUM_BLOCKS_TO_RECEIVE)
            if blk is None:
                counter += 1
                time.sleep(1)
                # stop retrying after 20 times
                if counter == 20:
                    break
                continue

            break


        recent_blks = seqrpc.alp_getRecentBlocks(NUM_BLOCKS_TO_RECEIVE)

        assert len(recent_blks) == NUM_BLOCKS_TO_RECEIVE

        # check if they are in order by verifying if N-1 block is parent of N block
        for idx in reversed(range(0, NUM_BLOCKS_TO_RECEIVE)):
            if idx != NUM_BLOCKS_TO_RECEIVE - 1:
                assert recent_blks[idx]["prev_block"] == recent_blks[idx + 1]["block_id"]

        l2_blk = seqrpc.alp_getBlocksAtIdx(recent_blks[BLOCK_NUMBER]["block_idx"])

        assert recent_blks[BLOCK_NUMBER]["block_idx"] == l2_blk[0]["block_idx"]

        second_blk_from_id = seqrpc.alp_getBlockById(l2_blk[0]["block_id"])

        # check if we got the correct block when looked using hash
        assert second_blk_from_id["block_id"] == l2_blk[0]["block_id"]
