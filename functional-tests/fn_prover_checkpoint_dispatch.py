import time

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the some block building
        time.sleep(4)

        checkpoint_idx = 1
        l1_range = (1, 3)
        l2_range = (1, 3)
        rpc_res = prover_client_rpc.dev_strata_proveCheckpointRaw(
            checkpoint_idx, l1_range, l2_range
        )
        print("got the rpc res: {}", rpc_res)
        assert rpc_res is not None

        time.sleep(10)
