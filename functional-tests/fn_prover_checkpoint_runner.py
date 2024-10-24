import time

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        # Wait for the Prover Manager setup
        time.sleep(60)

        # Test on with the latest checkpoint
        print("Waiting for checkpoint runner")

        time_out = 30 * 60
        time.sleep(time_out)
