import time

from bitcoinlib.services.bitcoind import BitcoindClient

import flexitest
from constants import BLOCK_GENERATION_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    REORG_DEPTH = 3

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("l1_read_reorg_test")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # We need at least `REORG_DEPTH` + 1 or more blocks
        # to invalidate `REORG_DEPTH` blocks at the end.
        wait_time = BLOCK_GENERATION_INTERVAL_SECS * (self.REORG_DEPTH + 1) + SEQ_SLACK_TIME_SECS
        time.sleep(wait_time)
        l1stat = seqrpc.alp_l1status()

        height_to_invalidate_from = int(l1stat["cur_height"]) - self.REORG_DEPTH
        block_to_invalidate_from = btcrpc.proxy.getblockhash(height_to_invalidate_from)
        to_be_invalid_block = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)
        btcrpc.proxy.invalidateblock(block_to_invalidate_from)

        # Wait for at least 1 block to be added after invalidating `REORG_DEPTH` blocks.
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 1 + SEQ_SLACK_TIME_SECS)
        block_from_invalidated_height = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)

        assert to_be_invalid_block != block_from_invalidated_height, "Expected reorg from 3rd Block"
