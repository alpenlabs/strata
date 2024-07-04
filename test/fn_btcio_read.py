import time
import flexitest


@flexitest.register
class L1StatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        btcrpc = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # proto_ver = seqrpc.alp_protocolVersion()
        # print("protocol version", proto_ver)
        # assert proto_ver == 1, "query protocol version"
        # add 5 blocks
        btc.generate_blocks(btcrpc, 0.05, 5)
        time.sleep(1)
        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)
        # check if current_height > 0
        assert l1stat["cur_height"] > 0, "Sequencer is not seeing L1 blocks"
