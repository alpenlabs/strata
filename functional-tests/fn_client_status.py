import time

import flexitest

from constants import MAX_HORIZON_POLL_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


@flexitest.register
class L1ClientStatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        proto_ver = seqrpc.alp_protocolVersion()
        print("protocol version", proto_ver)
        assert proto_ver == 1, "query protocol version"

<<<<<<<< HEAD:functional-tests/fn_client_status.py
        time.sleep(MAX_HORIZON_POLL_INTERVAL_SECS + SEQ_SLACK_TIME_SECS)
        client_status = seqrpc.alp_clientStatus()
        print("client status", client_status)
========
        # This sleep is needed to allow sequencer to boot up
        time.sleep(1)

        client_status = seqrpc.alp_clientStatus()
        print("client status", client_status)

        time.sleep(2)

        return True
>>>>>>>> c1d1ca3 (test: Group func tests in directories and update test module loading):test/sequencer/fn_client_status.py
