import flexitest

@flexitest.register
class HelloTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        proto_ver = seqrpc.alp_protocolVersion()
        print("protocol version", proto_ver)
        assert proto_ver == 1, "query protocol version"

        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)
