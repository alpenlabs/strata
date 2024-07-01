from time import sleep
import flexitest
import time
from bitcoinlib.services.bitcoind import BitcoindClient
from block_generator import generate_blocks

@flexitest.register
class L1ConnectTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        
        btcrpc = btc.create_rpc()
        seqrpc = seq.create_rpc()

        generate_blocks(btcrpc,1,block_count=1)
        #wait for block to be made
        time.sleep(0.5)
        l1stat = seqrpc.alp_l1connected()
        assert l1stat == True, "Error connecting to Bitcoin Rpc client"
        time.sleep(1)
        


