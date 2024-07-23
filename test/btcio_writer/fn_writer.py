import time
from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


@flexitest.register
class L1WriterTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        # btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        # btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # This sleep is needed to allow sequencer to boot up
        time.sleep(1)

        val = seqrpc.alp_testFunctional()
        print(val)
        assert val == 1
