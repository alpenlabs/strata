import logging
import time
from pathlib import Path

import flexitest

from utils import wait_for_proof_with_time_out
from setup import StrataTest


@flexitest.register
class ProverClientTest(StrataTest):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(60)

        # Dispatch the prover task
        task_id = prover_client_rpc.dev_strata_proveBtcBlock(1)
        self.debug(f"got the task id: {task_id}")
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
