import time
from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()
        
        time.sleep(2)
        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)

        second_block = btcrpc.proxy.getblockhash(2)
        third_block = btcrpc.proxy.getblockhash(3)
        btcrpc.proxy.invalidateblock(second_block)
        time.sleep(1)
        new_third_block = btcrpc.proxy.getblockhash(3)

        assert (
            third_block != new_third_block 
        ), "The 3rd block was not invalidated, which means the reorg didn't happen"
