import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import BLOCK_GENERATION_INTERVAL_SECS, SEQ_SLACK_TIME_SECS
from entry import BasicEnvConfig

REORG_DEPTH = 3


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        # standalone env for this test as it involves mutating the blockchain via invalidation
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # We need at least `REORG_DEPTH` + 1 or more blocks
        # to invalidate `REORG_DEPTH` blocks at the end.
        wait_time = BLOCK_GENERATION_INTERVAL_SECS * (REORG_DEPTH + 1) + SEQ_SLACK_TIME_SECS
        time.sleep(wait_time)
        l1stat = seqrpc.alp_l1status()

        height_to_invalidate_from = int(l1stat["cur_height"]) - REORG_DEPTH
        print("height to invalidate from", height_to_invalidate_from)
        block_to_invalidate_from = btcrpc.proxy.getblockhash(height_to_invalidate_from)
        to_be_invalid_block = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)
        print("invalidating block", to_be_invalid_block)
        btcrpc.proxy.invalidateblock(block_to_invalidate_from)

        # Wait for at least 1 block to be added after invalidating `REORG_DEPTH` blocks.
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 1 + SEQ_SLACK_TIME_SECS)
        block_from_invalidated_height = seqrpc.alp_getL1blockHash(height_to_invalidate_from + 1)
        print("now have block", block_from_invalidated_height)

        assert to_be_invalid_block != block_from_invalidated_height, "Expected reorg from block 3"
