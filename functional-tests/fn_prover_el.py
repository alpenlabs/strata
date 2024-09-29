import time

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        time.sleep(5)

        # Wait for the some block building
        task_id = prover_client_rpc.dev_alp_proveCLBlock(1)
        task_id = prover_client_rpc.dev_alp_proveCLBlock(2)

        task_id = prover_client_rpc.dev_alp_proveELBlock(1)
        task_id = prover_client_rpc.dev_alp_proveELBlock(2)

        # assert task_id is not None
        print("got the rpc res: {}", task_id)
