import time

import flexitest

from setup import TestStrata
from utils import wait_until

REORG_DEPTH = 3


@flexitest.register
class CLBlockWitnessDataGenerationTest(TestStrata):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        witness_1 = seqrpc.strata_getCLBlockWitness(1)
        assert witness_1 is not None

        time.sleep(1)
        witness_2 = seqrpc.strata_getCLBlockWitness(2)
        assert witness_2 is not None

        return True
