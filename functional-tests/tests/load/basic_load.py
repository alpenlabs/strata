import time

import flexitest

from envs import testenv

REORG_DEPTH = 3


@flexitest.register
class BasicLoadGenerationTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("load")

    def main(self, ctx: flexitest.RunContext):
        print("test is running")
        load = ctx.get_service("load_generator")
        _loadrpc = load.create_rpc()
        time.sleep(5)

        print("test is running")

        return True
