import time

import flexitest

from constants import SEQ_SLACK_TIME_SECS

REORG_DEPTH = 3


@flexitest.register
class CLBlockWitnessDataGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        time.sleep(SEQ_SLACK_TIME_SECS)

        witness_1 = seqrpc.alp_getBlockWitness(1)
        assert witness_1 is not None

        witness_2 = seqrpc.alp_getBlockWitness(2)
        assert witness_2 is not None

        return True
