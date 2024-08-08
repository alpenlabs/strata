import time

import flexitest

from constants import MAX_HORIZON_POLL_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


@flexitest.register
class ElBlockGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        rethrpc = reth.create_rpc()

        last_blocknum = int(rethrpc.eth_blockNumber(), 16);

        for _ in range(5):
            time.sleep(3)
            blocknum = int(rethrpc.eth_blockNumber(), 16)
            print("cur blocknum", blocknum)
            assert blocknum > last_blocknum, "seem to not be making progress"
            last_blocknum = blocknum
