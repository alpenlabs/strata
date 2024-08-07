import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest

UNSET_ID = "0000000000000000000000000000000000000000000000000000000000000000"
MAX_GENESIS_TRIES = 10

@flexitest.register
class SyncGenesisTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        time.sleep(3)

        # Wait until genesis.  This might need to be tweaked if we change how
        # long we wait for genesis in tests.
        tries = 0
        last_slot = None
        while True:
            if tries > MAX_GENESIS_TRIES:
                assert False, "did not observe genesis before timeout"

            print("waiting for genesis")
            stat = seqrpc.alp_clientStatus()
            print(stat)
            if stat["finalized_blkid"] != UNSET_ID:
                last_slot = stat["chain_tip_slot"]
                print("observed genesis, now at slot", last_slot)
                break

            time.sleep(0.5)
            print("waiting for genesis... -- tries", tries)

        assert last_slot is not None, "last slot never set"

        # Make sure we're making progress.
        stat = None
        for _ in range(5):
            time.sleep(3)
            stat = seqrpc.alp_clientStatus()
            tip_slot = stat["chain_tip_slot"]
            tip_blkid = stat["chain_tip"]
            print("cur tip slot", tip_slot, "blkid", tip_blkid)
            assert tip_slot >= last_slot, "cur slot went backwards"
            assert tip_slot > last_slot, "seem to not be making progress"
            last_slot = tip_slot
