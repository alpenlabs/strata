from functools import partial

import flexitest

import testenv
from utils import wait_until_with_value


@flexitest.register
class ElBlockGenerationTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(1000))

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        rethrpc = reth.create_rpc()

        last_blocknum = int(rethrpc.eth_blockNumber(), 16)
        for _ in range(5):
            cur_block_num = wait_until_with_value(
                lambda: int(rethrpc.eth_blockNumber(), 16),
                partial(lambda x, i: x < i, last_blocknum),
                error_with="seem not to be making progress",
            )
            last_blocknum = cur_block_num
