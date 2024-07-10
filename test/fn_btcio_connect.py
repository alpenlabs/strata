from time import sleep
import flexitest
import time


@flexitest.register
class L1ConnectTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        time.sleep(2)
        l1stat = seqrpc.alp_l1connected()
        print(l1stat)
        assert l1stat == True, "Error connecting to Bitcoin Rpc client"
