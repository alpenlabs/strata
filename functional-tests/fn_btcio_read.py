import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import MAX_HORIZON_POLL_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


@flexitest.register
class L1StatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        start_time = int(time.time())

        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()
        interval = MAX_HORIZON_POLL_INTERVAL_SECS + SEQ_SLACK_TIME_SECS

        time.sleep(interval)
        received_block = btcrpc.getblock(btcrpc.proxy.getbestblockhash())
        l1stat = seqrpc.alp_l1status()

        # Time is in millis
        cur_time = l1stat['last_update'] // 1000

        # ensure that the l1reader task has started within few seconds of test being run
        assert cur_time - start_time <= interval, "time not flowing properly"
        # check if height on bitcoin is same as, it is seen in sequencer
        assert (
            l1stat["cur_height"] == received_block["height"]
        ), "sequencer height doesn't match the bitcoin node height"

        time.sleep(MAX_HORIZON_POLL_INTERVAL_SECS * 2)
        l1stat = seqrpc.alp_l1status()
        elapsed_time = l1stat['last_update'] // 1000

        # check if L1 reader is seeing new L1 activity
        assert elapsed_time - cur_time >= MAX_HORIZON_POLL_INTERVAL_SECS * 2, "time not flowing properly"
