import time

import flexitest

from utils import wait_until

REORG_DEPTH = 3


@flexitest.register
class CLBlockWitnessDataGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        # Wait for seq
        wait_until(
            lambda: seqrpc.alp_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )
        time.sleep(1)

        witness_1 = seqrpc.alp_getCLBlockWitness(1)
        assert witness_1 is not None
        print("got the block witness ", witness_1)

        time.sleep(1)
        witness_2 = seqrpc.alp_getCLBlockWitness(2)
        assert witness_2 is not None
        print("got the block witness ", witness_2)

        time.sleep(1)
        witness_3 = seqrpc.alp_getCLBlockWitness(3)
        assert witness_3 is not None
        print("got the block witness ", witness_3)

        return True
