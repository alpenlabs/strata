import time

import flexitest

from envs import testenv


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(5)

        for i in range(30):
            print("\n\n Step: ", i)
            block_time = seqrpc.strata_blockTime()
            print("block_time ", block_time)

            time.sleep(10)
