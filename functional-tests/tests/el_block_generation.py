import logging
import time
from functools import partial

import flexitest

from envs import testenv
from utils import wait_until_with_value


@flexitest.register
class ElBlockGenerationTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(1000))

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        rethrpc = reth.create_rpc()

        last_blocknum = int(rethrpc.eth_blockNumber(), 16)
        logging.info(f"initial EL blocknum is {last_blocknum}")

        for _ in range(5):
            time.sleep(3)
            cur_blocknum = int(rethrpc.eth_blockNumber(), 16)
            logging.info(f"current EL blocknum is {cur_blocknum}")
            assert cur_blocknum >= last_blocknum, "cur block went backwards"
            assert cur_blocknum > last_blocknum, "seem to not be making progress"
            last_blocknum = cur_blocknum
