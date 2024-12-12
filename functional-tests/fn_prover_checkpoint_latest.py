import time

import flexitest

from setup import TestStrata
from utils import wait_for_proof_with_time_out


@flexitest.register
class ProverClientTest(TestStrata):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(60)

        # Test on with the latest checkpoint
        task_id = prover_client_rpc.dev_strata_proveLatestCheckPoint()
        self.debug(f"got the task id: {task_id}")
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
