import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest
from seqrpc import RpcError


NO_OF_BLOCKS_TO_RECEIVE = 10
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
            try:
                _ = seqrpc.alp_getBlocksAtIdx(NO_OF_BLOCKS_TO_RECEIVE)
            except RpcError:
                # cancel retrying if we encounter this for more than 20 times
                if counter == 20:
                    break
                time.sleep(1)
                counter += 1
            break


        recent_blks = seqrpc.alp_getRecentBlocks(NO_OF_BLOCKS_TO_RECEIVE)

        assert len(recent_blks) == NO_OF_BLOCKS_TO_RECEIVE

        # check if they are in order by verifying if N-1 block is parent of N block
        for idx in range(0,NO_OF_BLOCKS_TO_RECEIVE):
            if idx != NO_OF_BLOCKS_TO_RECEIVE-1:
                assert recent_blks[idx]["prev_block"] == recent_blks[idx+1]["block_id"]


        l2_blk = seqrpc.alp_getBlocksAtIdx(recent_blks[BLOCK_NUMBER]["block_idx"])

        assert recent_blks[BLOCK_NUMBER]["block_idx"] == l2_blk[0]["block_idx"]

        second_blk_from_id = seqrpc.alp_getBlockById(l2_blk[0]["block_id"])

        # check if we got the correct block when looked using hash
        assert second_blk_from_id["block_id"] == l2_blk[0]["block_id"]





