import logging
import time
from pathlib import Path

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        # Wait for the Prover Manager setup
        time.sleep(60)

        # Test on with the latest checkpoint
        self.logger.debug("Waiting for checkpoint runner")

        time_out = 10 * 60
        time.sleep(time_out)
