import time
from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


@flexitest.register
class L1StatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        time.sleep(1)
        received_block = btcrpc.getblock(btcrpc.proxy.getbestblockhash())
        l1stat = seqrpc.alp_l1status()

        assert (
            l1stat["cur_height"] == received_block["height"]
        ), "Height seen by Sequencer doesn't match the Height on the bitcoin node"
