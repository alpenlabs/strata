import time

import flexitest

from utils import wait_for_proof_with_time_out


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the some block building
        time.sleep(60)

        task_id = prover_client_rpc.dev_strata_proveL1Batch((1, 2))
        print("got the task id: {}", task_id)
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
