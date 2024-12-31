import flexitest
import time

import testenv


@flexitest.register
class RPCBailTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        self.debug("checking connectivity")
        l1stat = seqrpc.strata_l1connected()
        assert l1stat, "Error connecting to Bitcoin Rpc client"
        time.sleep(2)

        ut = seqrpc.stratadebug_bail("test")
        print("check after 5 sec if it went to sleep")
        time.sleep(5)
        l1stat = seqrpc.strata_l1connected()

