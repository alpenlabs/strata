import time

import flexitest


@flexitest.register
class FullnodeElBlockGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("fullnode")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()
        fullnode_reth = ctx.get_service("fullnode_reth")
        fullnode_reth_rpc = fullnode_reth.create_rpc()

        # give some time for the sequencer to start up and generate blocks
        time.sleep(3)

        last_blocknum = int(rethrpc.eth_blockNumber(), 16)

        # test an older block because latest may not have been synced yet
        test_blocknum = last_blocknum - 1

        assert test_blocknum > 0, "not enough blocks generated"

        block_from_sequencer = rethrpc.eth_getBlockByNumber(hex(test_blocknum), False)
        block_from_fullnode = fullnode_reth_rpc.eth_getBlockByNumber(hex(test_blocknum), False)

        assert block_from_sequencer == block_from_fullnode, "blocks don't match"
