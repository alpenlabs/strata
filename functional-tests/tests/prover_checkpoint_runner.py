import time

import flexitest

from envs import testenv
from utils import wait_for_proof_with_time_out


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        self.warning("SKIPPING TEST prover_checkpoint_manual - not implemented")
        return True
