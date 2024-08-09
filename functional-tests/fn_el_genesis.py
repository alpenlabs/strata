import time

import flexitest

from constants import MAX_HORIZON_POLL_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


@flexitest.register
class ElGenesisTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        rethrpc = reth.create_rpc()
        genesis_block = rethrpc.eth_getBlockByNumber(hex(0), True)

        assert genesis_block["hash"] == "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba", "genesis block hash"
