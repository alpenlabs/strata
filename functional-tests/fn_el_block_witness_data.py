import random
import time

import flexitest


@flexitest.register
class ElBlockWitnessDataGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        rethrpc = reth.create_rpc()

        # TODO: do some transactions

        time.sleep(6)

        last_blocknum = int(rethrpc.eth_blockNumber(), 16)

        assert last_blocknum > 5, "dont have enough blocks generated"

        blocknums = random.sample(range(1, last_blocknum + 1), 5)

        for blocknum in blocknums:
            blockhash = rethrpc.eth_getBlockByNumber(hex(blocknum), False)["hash"]
            witness_data = rethrpc.alpee_getBlockWitness(blockhash, False)

            assert witness_data is not None, "non empty witness"

            # TODO: check witness data is ok ?

            print(blocknum, witness_data)
