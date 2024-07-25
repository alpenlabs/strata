import time

import flexitest
from constants import BLOCK_GENERATION_INTERVAL_SECS, SEQ_SLACK_TIME_SECS


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

        time.sleep(BLOCK_GENERATION_INTERVAL_SECS + SEQ_SLACK_TIME_SECS)
        client_status = seqrpc.alp_clientStatus()
        print("client status", client_status)
