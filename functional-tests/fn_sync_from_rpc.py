import time

import flexitest


@flexitest.register
class FullnodeElBlockGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("hub1")

    def main(self, ctx: flexitest.RunContext):
        seq_reth_rpc = ctx.get_service("seq_reth").create_rpc()
        fullnode_reth_rpc = ctx.get_service("follower_1_reth").create_rpc()

        # give some time for the sequencer to start up and generate blocks
        time.sleep(3)

        last_blocknum = int(seq_reth_rpc.eth_blockNumber(), 16)

        # test an older block because latest may not have been synced yet
        test_blocknum = last_blocknum - 1

        assert test_blocknum > 0, "not enough blocks generated"

        block_from_sequencer = seq_reth_rpc.eth_getBlockByNumber(hex(test_blocknum), False)
        block_from_fullnode = fullnode_reth_rpc.eth_getBlockByNumber(hex(test_blocknum), False)

        assert block_from_sequencer == block_from_fullnode, "blocks don't match"
