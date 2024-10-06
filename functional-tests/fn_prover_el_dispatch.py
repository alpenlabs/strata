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
        time.sleep(60)

        for i in range(1, 1 + 1):
            rpc_res = prover_client_rpc.dev_strata_proveELBlock(i)
            # rpc_res = prover_client_rpc.dev_strata_proveELBlock(i)
            print("got the rpc res: {} for el block {}", rpc_res, i)
            assert rpc_res is not None

        time.sleep(400)
