import time

import flexitest


@flexitest.register
class L1ConnectTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("premined_blocks")

    def main(self, ctx: flexitest.RunContext):
        while True:
            time.sleep(2)
