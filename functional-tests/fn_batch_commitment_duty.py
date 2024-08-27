import time

import flexitest


@flexitest.register
class L1StatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("fast_batches")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        # NOTE: this test checks for batches being published,
        # and assumes all txns are for batch (valid at this time)
        # may need more detailed stats for type of published transaction

        l1stat = None
        for _ in range(10):
            time.sleep(1)
            l1stat = seqrpc.alp_l1status()
            if l1stat["published_inscription_count"] > 0:
                print("saw published txn", l1stat["last_published_txid"])
                break
        else:
            raise AssertionError("did not see batches being published")

        last_l1stat = l1stat
        initial_txn_count = last_l1stat["published_inscription_count"]
        final_txn_count = initial_txn_count

        for _ in range(5):
            time.sleep(6)

            l1stat = seqrpc.alp_l1status()
            print(l1stat)
            if l1stat["published_inscription_count"] <= last_l1stat["published_inscription_count"]:
                print("no published transactions")

            final_txn_count = l1stat["published_inscription_count"]

        # batches should be produced ~ every 5s
        assert final_txn_count - initial_txn_count > 5, "not enough batches produced"
