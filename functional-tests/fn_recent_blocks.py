import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest



@flexitest.register
class RecentBlocksTest(flexitest.Test):
    NO_OF_BLOCKS_TO_RECEIVE = 3
    BLOCK_NUMBER = 2
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        time.sleep(2)
        recent_blks = seqrpc.alp_getRecentBlocks(self.NO_OF_BLOCKS_TO_RECEIVE)

        assert len(recent_blks) == self.NO_OF_BLOCKS_TO_RECEIVE

        second_blk = seqrpc.alp_getBlocksAtIdx(recent_blks[self.BLOCK_NUMBER]["block_idx"])

        assert recent_blks[self.BLOCK_NUMBER]["block_idx"] == second_blk[0]["block_idx"]

        second_blk_from_id = seqrpc.alp_getBlockById(second_blk[0]["block_id"])
        # check if we got the correct block when looked  using hash
        assert second_blk_from_id["block_id"] == second_blk[0]["block_id"]





