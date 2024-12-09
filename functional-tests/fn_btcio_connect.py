import logging
from pathlib import Path

import flexitest


@flexitest.register
class L1ConnectTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        self.logger.debug("checking connectivity")
        l1stat = seqrpc.strata_l1connected()
        assert l1stat, "Error connecting to Bitcoin Rpc client"
