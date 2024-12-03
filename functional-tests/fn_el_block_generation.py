import logging
from functools import partial
from pathlib import Path

import flexitest

from entry import BasicEnvConfig
from utils import wait_until_with_value


@flexitest.register
class ElBlockGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(1000))
        self.logger = logging.getLogger(Path(__file__).stem)

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
