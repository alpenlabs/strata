import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest

UNSET_ID = "0000000000000000000000000000000000000000000000000000000000000000"


@flexitest.register
class SyncGenesisTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        time.sleep(1)

        stat = None
        for _ in range(10):
            stat = seqrpc.alp_clientStatus()
            print(stat)
            time.sleep(1)

        assert stat["finalized_blkid"] != UNSET_ID, "did not notice genesis"
