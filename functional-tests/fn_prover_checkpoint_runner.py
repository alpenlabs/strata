import logging
import time
from pathlib import Path

import flexitest

from setup import TestStrata


@flexitest.register
class ProverClientTest(TestStrata):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        # Wait for the Prover Manager setup
        time.sleep(60)

        # Test on with the latest checkpoint
        self.debug("Waiting for checkpoint runner")

        time_out = 10 * 60
        time.sleep(time_out)
