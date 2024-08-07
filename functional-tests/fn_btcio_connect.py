import flexitest


@flexitest.register
class L1ConnectTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        print("checking connectivity")
        l1stat = seqrpc.alp_l1connected()
        assert l1stat, "Error connecting to Bitcoin Rpc client"
