import time

from bitcoinlib.services.bitcoind import BitcoindClient

import flexitest


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("l1_read_reorg_test")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # Add two blocks since each block is generated every 0.5 seconds and we need at least 4
        # or more blocks to invalidate 3 blocks at the end
        time.sleep(3)
        l1stat = seqrpc.alp_l1status()
        # relative height is chosen such that we can have arbitrary number of blocks
        # which is affected by the time.sleep above
        # blocks n-2 , n-1 , n are invalidated where n is the height of blockchain
        height_to_invalidate_from = int(l1stat["cur_height"]) - 3
        block_to_invalidate_from = btcrpc.proxy.getblockhash(height_to_invalidate_from)
        to_be_invalid_block = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)
        btcrpc.proxy.invalidateblock(block_to_invalidate_from)
        # Wait for some blocks to be added after invalidating (n-3) blocks
        # because poll time for sequencer is supposed to be 500ms
        # and 2 seconds seems to be optimal for sequencer to catch changes
        time.sleep(2)
        block_from_invalidated_height = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)
        assert to_be_invalid_block != block_from_invalidated_height, "Expected reorg from 3rd Block"
