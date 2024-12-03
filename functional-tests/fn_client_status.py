import logging
from pathlib import Path

import flexitest

from utils import wait_until


@flexitest.register
class L1ClientStatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        proto_ver = seqrpc.strata_protocolVersion()
        self.logger.debug(f"protocol version { proto_ver}")
        assert proto_ver == 1, "query protocol version"

        client_status = seqrpc.strata_clientStatus()
        self.logger.debug(f"client status { client_status}")

        return True
